use kurbo::Shape as _;
use renamite_machine::{Condition, InputValue, MachineInstance, StateKind};
use renamite_model::{evaluate_with, Overrides, Value};

fn load() -> renamite_io_ren::RenFile {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anims/zombie.ren");
    let text = std::fs::read_to_string(path).unwrap();
    renamite_io_ren::open(&text).unwrap()
}

fn node(file: &renamite_io_ren::RenFile, name: &str) -> renamite_model::NodeId {
    file.document
        .find_nodes_by_name(name)
        .next()
        .unwrap_or_else(|| panic!("node {name} not found"))
}

#[test]
fn walk_clip_drives_pelvis_bob() {
    let file = load();
    let pelvis = node(&file, "pelvis");
    let walk = file
        .clips
        .values()
        .find(|c| c.name == "walk")
        .expect("walk clip");

    let mut a = Overrides::default();
    walk.sample_into(0.0, &mut a.values);
    let mut b = Overrides::default();
    walk.sample_into(15.0, &mut b.values);

    match (
        a.get(pelvis, "transform.position"),
        b.get(pelvis, "transform.position"),
    ) {
        (Some(Value::DVec2(p0)), Some(Value::DVec2(p1))) => {
            assert_eq!(p0.y, 190.0);
            assert_eq!(p1.y, 185.0);
            assert_eq!(p0.x, 128.0);
        }
        other => panic!("pelvis position overrides missing: {other:?}"),
    }

    let leg_l = node(&file, "leg_l");
    match (
        a.get(leg_l, "transform.rotation"),
        b.get(leg_l, "transform.rotation"),
    ) {
        (Some(Value::Angle(r0)), Some(Value::Angle(r1))) => {
            assert_eq!(r0.0, -20.0);
            assert_eq!(r1.0, 2.0);
        }
        other => panic!("leg rotation overrides missing: {other:?}"),
    }
}

#[test]
fn machine_walk_eat_fall_transitions_and_events() {
    let file = load();
    let machine = file
        .machines
        .get(file.start_machine.expect("start machine"))
        .unwrap();
    let clips = &file.clips;
    let mut inst = MachineInstance::new(machine);
    let mut ov = Overrides::default();

    assert!(matches!(inst.inputs[0], InputValue::Bool(false)));

    inst.tick(machine, clips, 1.0, &mut ov);
    assert_eq!(state(&inst), 0);

    inst.set_bool(0, true);
    inst.tick(machine, clips, 1.0, &mut ov);
    assert_eq!(state(&inst), 1, "eating=true must enter eat state");

    let mut events = Vec::new();
    for _ in 0..30 {
        events.extend(inst.tick(machine, clips, 1.0, &mut ov).events);
    }
    assert!(
        events.contains(&"bite".to_string()),
        "bite event must fire: {events:?}"
    );

    let jaw = node(&file, "jaw");
    let open = ov
        .get(jaw, "transform.rotation")
        .map(|v| matches!(v, Value::Angle(a) if a.0 > 0.0))
        .unwrap_or(false);
    assert!(open, "jaw must be keyed open during eat");

    inst.set_bool(0, false);
    inst.tick(machine, clips, 1.0, &mut ov);
    assert_eq!(state(&inst), 0, "eating=false must return to walk");

    let die_index = machine
        .inputs
        .iter()
        .position(|i| i.name == "die")
        .expect("die input");
    inst.fire(die_index);
    inst.tick(machine, clips, 1.0, &mut ov);
    assert_eq!(state(&inst), 2, "die trigger must enter fall state");

    let mut events = Vec::new();
    for _ in 0..50 {
        events.extend(inst.tick(machine, clips, 1.0, &mut ov).events);
    }
    for expected in ["arm_off", "thud", "died"] {
        assert!(
            events.iter().any(|e| e == expected),
            "missing {expected} in {events:?}"
        );
    }

    let pelvis = node(&file, "pelvis");
    let rot = ov
        .get(pelvis, "transform.rotation")
        .map(|v| matches!(v, Value::Angle(a) if a.0 > 70.0))
        .unwrap_or(false);
    assert!(rot, "pelvis must be toppled at end of fall");
}

#[test]
fn fall_never_reenters_from_lingering_trigger() {
    let file = load();
    let machine = file.machines.get(file.start_machine.unwrap()).unwrap();
    let mut inst = MachineInstance::new(machine);
    let mut ov = Overrides::default();

    let die_index = machine.inputs.iter().position(|i| i.name == "die").unwrap();
    inst.fire(die_index);
    inst.tick(machine, &file.clips, 1.0, &mut ov);
    for _ in 0..20 {
        inst.tick(machine, &file.clips, 1.0, &mut ov);
    }
    assert_eq!(state(&inst), 2);
    let pelvis = node(&file, "pelvis");
    let rot = ov
        .get(pelvis, "transform.rotation")
        .map(|v| matches!(v, Value::Angle(a) if a.0 > 10.0))
        .unwrap_or(false);
    assert!(
        rot,
        "fall must keep advancing; a re-entry loop would pin the clip at frame 0"
    );
}

fn state(inst: &MachineInstance) -> usize {
    inst.layer_states().next().unwrap()
}

#[test]
fn overrides_change_evaluated_scene() {
    let file = load();
    let walk = file.clips.values().find(|c| c.name == "walk").unwrap();

    let mut ov0 = Overrides::default();
    walk.sample_into(0.0, &mut ov0.values);
    let mut ov15 = Overrides::default();
    walk.sample_into(15.0, &mut ov15.values);

    let s0 = evaluate_with(&file.document, file.document.main, 0.0, &ov0);
    let s15 = evaluate_with(&file.document, file.document.main, 0.0, &ov15);

    assert!(
        s0.items.len() >= 10,
        "expected all visible parts, got {}",
        s0.items.len()
    );

    let bb = |s: &renamite_model::Scene| {
        s.items
            .iter()
            .map(|i| i.path.bounding_box())
            .fold(f64::INFINITY, |acc, b| acc.min(b.y1))
    };
    let y0 = bb(&s0);
    let y15 = bb(&s15);
    assert!(
        (y15 - y0).abs() > 1.0,
        "scene must move between walk frames 0 and 15 (y1 {y0} vs {y15})"
    );
}

#[test]
fn armor_accessories_hidden_by_default() {
    let file = load();
    for name in ["cone", "bucket"] {
        let n = &file.document.nodes[node(&file, name)];
        assert!(!n.visible, "{name} must ship hidden");
    }
}

#[test]
fn machine_conditions_reference_valid_indices() {
    let file = load();
    let machine = file.machines.get(file.start_machine.unwrap()).unwrap();
    let layer = &machine.layers[0];
    assert_eq!(machine.inputs.len(), 2);
    assert_eq!(machine.inputs[0].name, "eating");
    assert_eq!(machine.inputs[1].name, "die");
    assert_eq!(layer.entry, 0);
    assert_eq!(layer.states.len(), 3);

    let check = |conds: &[Condition]| {
        for c in conds {
            let idx = match *c {
                Condition::BoolIs { input, .. } | Condition::Triggered { input } => input,
                Condition::NumberCmp { input, .. } => input,
            };
            assert!(idx < machine.inputs.len());
        }
    };
    for st in &layer.states {
        for tr in &st.transitions {
            assert!(tr.to < layer.states.len());
            check(&tr.conditions);
        }
    }
    for tr in &layer.any_transitions {
        assert!(tr.to < layer.states.len());
        check(&tr.conditions);
    }

    let fall = &layer.states[2];
    assert!(matches!(
        fall.kind,
        StateKind::Clip {
            loop_mode: renamite_animation::LoopMode::Once,
            ..
        }
    ));
}
