use glam::DVec2;
use renamite_animation::{
    Angle, Animated, EasingHandle, EasingPreset, Frame, FrameRate, Interpolation, LoopMode,
};
use renamite_geometry::{Anchor, VectorPath};
use renamite_io_ren::RenFile;
use renamite_machine::{
    Clip, ClipMap, Condition, EventKey, InputDef, InputKind, Machine, MachineLayer, MachineMap,
    State, StateKind, Track, Transition,
};
use renamite_model::{
    Document, FillRule, KeyframeData, Node, NodeId, NodeKind, Parent, PropPath, ShapeKind,
    StyleKind, StylePaint, Value,
};

const INK: f64 = 0.10;
const DARK: f64 = 0.30;
const MID: f64 = 0.55;
const LIGHT: f64 = 0.75;

fn gray(v: f64) -> Color {
    Color::rgba(v, v, v, 1.0)
}

use renamite_model::Color;

fn kf(frame: i64, value: Value, preset: EasingPreset) -> KeyframeData {
    let (interpolation, ease_out, ease_in): (Interpolation, EasingHandle, EasingHandle) =
        preset.segment();
    KeyframeData {
        frame: Frame(frame),
        value,
        interpolation,
        ease_out,
        ease_in,
    }
}

fn deg_value(d: f64) -> Value {
    Value::Angle(Angle(d))
}

fn vec_value(x: f64, y: f64) -> Value {
    Value::DVec2(DVec2::new(x, y))
}

fn track(node: NodeId, prop: &str, keys: Vec<KeyframeData>) -> Track {
    Track {
        node,
        prop: PropPath::new(prop),
        keys,
    }
}

fn rot_track(node: NodeId, keys: &[(i64, f64, EasingPreset)]) -> Track {
    track(
        node,
        "transform.rotation",
        keys.iter()
            .map(|(f, d, p)| kf(*f, deg_value(*d), *p))
            .collect(),
    )
}

fn pos_track(node: NodeId, keys: &[(i64, f64, f64, EasingPreset)]) -> Track {
    track(
        node,
        "transform.position",
        keys.iter()
            .map(|(f, x, y, p)| kf(*f, vec_value(*x, *y), *p))
            .collect(),
    )
}

fn opacity_track(node: NodeId, keys: &[(i64, f64, EasingPreset)]) -> Track {
    track(
        node,
        "opacity",
        keys.iter()
            .map(|(f, o, p)| kf(*f, Value::F64(*o), *p))
            .collect(),
    )
}

fn add(doc: &mut Document, parent: Parent, node: Node) -> NodeId {
    let id = doc.create_node(node);
    doc.attach(id, parent, usize::MAX).unwrap();
    id
}

fn joint(doc: &mut Document, parent: Parent, name: &str, x: f64, y: f64) -> NodeId {
    let mut n = Node::new(name, NodeKind::Group);
    n.transform.position = Animated::new(DVec2::new(x, y));
    n.transform.anchor = Animated::new(DVec2::new(x, y));
    add(doc, parent, n)
}

fn fill_style(doc: &mut Document, scope: NodeId, shade: f64) {
    add(
        doc,
        Parent::Node(scope),
        Node::new(
            "fill",
            NodeKind::Style(StyleKind::Fill {
                paint: StylePaint::solid(gray(shade)),
                rule: FillRule::NonZero,
            }),
        ),
    );
}

fn ellipse_part(
    doc: &mut Document,
    parent: Parent,
    name: &str,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    shade: f64,
) -> NodeId {
    let wrap = add(doc, parent, Node::new(name, NodeKind::Group));
    add(
        doc,
        Parent::Node(wrap),
        Node::new(
            "geo",
            NodeKind::Shape(ShapeKind::Ellipse {
                pos: Animated::new(DVec2::new(cx, cy)),
                size: Animated::new(DVec2::new(w, h)),
            }),
        ),
    );
    fill_style(doc, wrap, shade);
    wrap
}

fn path_part(
    doc: &mut Document,
    parent: Parent,
    name: &str,
    pts: &[(f64, f64)],
    shade: f64,
) -> NodeId {
    let wrap = add(doc, parent, Node::new(name, NodeKind::Group));
    let path = VectorPath {
        anchors: pts
            .iter()
            .map(|(x, y)| Anchor::corner(DVec2::new(*x, *y)))
            .collect(),
        closed: true,
    };
    add(
        doc,
        Parent::Node(wrap),
        Node::new("geo", NodeKind::Shape(ShapeKind::Path(Animated::new(path)))),
    );
    fill_style(doc, wrap, shade);
    wrap
}

pub struct ZombieRig {
    pub pelvis: NodeId,
    pub torso: NodeId,
    pub head: NodeId,
    pub jaw: NodeId,
    pub arm_l: NodeId,
    pub fore_l: NodeId,
    pub arm_r: NodeId,
    pub fore_r: NodeId,
    pub leg_l: NodeId,
    pub shin_l: NodeId,
    pub leg_r: NodeId,
    pub shin_r: NodeId,
}

pub fn build_rig(doc: &mut Document) -> ZombieRig {
    let root = Parent::Comp(doc.main);

    let pelvis = joint(doc, root, "pelvis", 128.0, 190.0);
    let pp = Parent::Node(pelvis);

    let torso = joint(doc, pp, "torso", 128.0, 190.0);
    let tp = Parent::Node(torso);

    path_part(
        doc,
        tp,
        "body",
        &[
            (106.0, 128.0),
            (152.0, 124.0),
            (148.0, 190.0),
            (110.0, 192.0),
        ],
        MID,
    );

    let arm_l = joint(doc, tp, "arm_l", 138.0, 134.0);
    path_part(
        doc,
        Parent::Node(arm_l),
        "upper_l",
        &[
            (132.0, 128.0),
            (186.0, 134.0),
            (186.0, 148.0),
            (132.0, 142.0),
        ],
        MID,
    );
    let fore_l = joint(doc, Parent::Node(arm_l), "fore_l", 186.0, 141.0);
    path_part(
        doc,
        Parent::Node(fore_l),
        "lower_l",
        &[
            (180.0, 135.0),
            (228.0, 141.0),
            (228.0, 153.0),
            (180.0, 147.0),
        ],
        MID,
    );

    let head = joint(doc, tp, "head", 128.0, 116.0);
    let hp = Parent::Node(head);

    let cone = path_part(
        doc,
        hp,
        "cone",
        &[(116.0, 68.0), (145.0, 68.0), (138.0, 26.0), (123.0, 26.0)],
        DARK,
    );
    doc.nodes[cone].visible = false;

    let bucket = add(
        doc,
        hp,
        Node::new(
            "bucket",
            NodeKind::Shape(ShapeKind::Rect {
                pos: Animated::new(DVec2::new(131.0, 60.0)),
                size: Animated::new(DVec2::new(48.0, 44.0)),
                rounded: Animated::new(5.0),
            }),
        ),
    );
    fill_style(doc, bucket, MID);
    doc.nodes[bucket].visible = false;

    ellipse_part(doc, hp, "eye", 141.0, 82.0, 9.0, 11.0, INK);
    ellipse_part(doc, hp, "skull", 130.0, 88.0, 54.0, 58.0, LIGHT);

    let jaw = joint(doc, hp, "jaw", 142.0, 96.0);
    path_part(
        doc,
        Parent::Node(jaw),
        "jawbone",
        &[(126.0, 96.0), (160.0, 94.0), (158.0, 112.0), (134.0, 110.0)],
        MID,
    );

    let arm_r = joint(doc, tp, "arm_r", 144.0, 128.0);
    path_part(
        doc,
        Parent::Node(arm_r),
        "upper_r",
        &[
            (140.0, 122.0),
            (194.0, 127.0),
            (194.0, 139.0),
            (140.0, 134.0),
        ],
        DARK,
    );
    let fore_r = joint(doc, Parent::Node(arm_r), "fore_r", 194.0, 133.0);
    path_part(
        doc,
        Parent::Node(fore_r),
        "lower_r",
        &[
            (188.0, 127.0),
            (236.0, 132.0),
            (236.0, 143.0),
            (188.0, 138.0),
        ],
        DARK,
    );

    let leg_l = joint(doc, pp, "leg_l", 120.0, 190.0);
    path_part(
        doc,
        Parent::Node(leg_l),
        "thigh_l",
        &[
            (113.0, 192.0),
            (127.0, 192.0),
            (125.0, 238.0),
            (115.0, 238.0),
        ],
        MID,
    );
    let shin_l = joint(doc, Parent::Node(leg_l), "shin_l", 120.0, 236.0);
    path_part(
        doc,
        Parent::Node(shin_l),
        "shin_l_geo",
        &[
            (114.0, 236.0),
            (126.0, 236.0),
            (126.0, 288.0),
            (146.0, 288.0),
            (146.0, 294.0),
            (114.0, 294.0),
        ],
        MID,
    );

    let leg_r = joint(doc, pp, "leg_r", 136.0, 190.0);
    path_part(
        doc,
        Parent::Node(leg_r),
        "thigh_r",
        &[
            (129.0, 192.0),
            (143.0, 192.0),
            (141.0, 238.0),
            (131.0, 238.0),
        ],
        DARK,
    );
    let shin_r = joint(doc, Parent::Node(leg_r), "shin_r", 136.0, 236.0);
    path_part(
        doc,
        Parent::Node(shin_r),
        "shin_r_geo",
        &[
            (130.0, 236.0),
            (142.0, 236.0),
            (142.0, 288.0),
            (162.0, 288.0),
            (162.0, 294.0),
            (130.0, 294.0),
        ],
        DARK,
    );

    ZombieRig {
        pelvis,
        torso,
        head,
        jaw,
        arm_l,
        fore_l,
        arm_r,
        fore_r,
        leg_l,
        shin_l,
        leg_r,
        shin_r,
    }
}

pub fn walk_clip(rig: &ZombieRig) -> Clip {
    let eio = EasingPreset::EaseInOut;
    Clip {
        name: "walk".into(),
        range: (Frame(0), Frame(60)),
        tracks: vec![
            pos_track(
                rig.pelvis,
                &[
                    (0, 128.0, 190.0, eio),
                    (15, 128.0, 185.0, eio),
                    (30, 128.0, 190.0, eio),
                    (45, 128.0, 185.0, eio),
                    (60, 128.0, 190.0, eio),
                ],
            ),
            rot_track(
                rig.torso,
                &[
                    (0, 7.0, eio),
                    (15, 11.0, eio),
                    (30, 7.0, eio),
                    (45, 11.0, eio),
                    (60, 7.0, eio),
                ],
            ),
            rot_track(
                rig.leg_l,
                &[(0, -20.0, eio), (30, 24.0, eio), (60, -20.0, eio)],
            ),
            rot_track(
                rig.shin_l,
                &[
                    (0, 10.0, eio),
                    (15, 26.0, eio),
                    (30, -6.0, eio),
                    (60, 10.0, eio),
                ],
            ),
            rot_track(
                rig.leg_r,
                &[(0, 24.0, eio), (30, -20.0, eio), (60, 24.0, eio)],
            ),
            rot_track(
                rig.shin_r,
                &[(0, -6.0, eio), (45, 26.0, eio), (60, -6.0, eio)],
            ),
            rot_track(rig.arm_r, &[(0, 5.0, eio), (30, -3.0, eio), (60, 5.0, eio)]),
            rot_track(
                rig.fore_r,
                &[(0, -4.0, eio), (30, 6.0, eio), (60, -4.0, eio)],
            ),
            rot_track(
                rig.arm_l,
                &[(0, -3.0, eio), (30, 5.0, eio), (60, -3.0, eio)],
            ),
            rot_track(
                rig.fore_l,
                &[(0, 6.0, eio), (30, -4.0, eio), (60, 6.0, eio)],
            ),
            rot_track(
                rig.head,
                &[(0, -8.0, eio), (30, -3.0, eio), (60, -8.0, eio)],
            ),
            rot_track(
                rig.jaw,
                &[
                    (0, 4.0, eio),
                    (20, 14.0, eio),
                    (40, 2.0, eio),
                    (60, 4.0, eio),
                ],
            ),
        ],
        events: vec![
            EventKey {
                frame: Frame(2),
                name: "footstep".into(),
            },
            EventKey {
                frame: Frame(32),
                name: "footstep".into(),
            },
        ],
    }
}

pub fn eat_clip(rig: &ZombieRig) -> Clip {
    let eio = EasingPreset::EaseInOut;
    Clip {
        name: "eat".into(),
        range: (Frame(0), Frame(30)),
        tracks: vec![
            rot_track(
                rig.jaw,
                &[
                    (0, 3.0, eio),
                    (8, 30.0, EasingPreset::EaseOut),
                    (14, 0.0, EasingPreset::EaseIn),
                    (30, 3.0, eio),
                ],
            ),
            rot_track(
                rig.head,
                &[
                    (0, -6.0, eio),
                    (8, -15.0, eio),
                    (14, -2.0, eio),
                    (30, -6.0, eio),
                ],
            ),
            rot_track(rig.torso, &[(0, 9.0, eio), (14, 13.0, eio), (30, 9.0, eio)]),
        ],
        events: vec![EventKey {
            frame: Frame(12),
            name: "bite".into(),
        }],
    }
}

pub fn fall_clip(rig: &ZombieRig) -> Clip {
    let ein = EasingPreset::EaseIn;
    Clip {
        name: "fall".into(),
        range: (Frame(0), Frame(45)),
        tracks: vec![
            rot_track(
                rig.fore_r,
                &[(0, 0.0, EasingPreset::Linear), (6, 80.0, ein)],
            ),
            pos_track(
                rig.fore_r,
                &[(6, 194.0, 133.0, ein), (16, 232.0, 320.0, ein)],
            ),
            opacity_track(
                rig.fore_r,
                &[(12, 1.0, EasingPreset::Hold), (18, 0.0, EasingPreset::Hold)],
            ),
            rot_track(
                rig.pelvis,
                &[(10, 0.0, EasingPreset::EaseInOut), (34, 78.0, ein)],
            ),
            pos_track(
                rig.pelvis,
                &[
                    (10, 128.0, 190.0, EasingPreset::EaseInOut),
                    (34, 120.0, 214.0, ein),
                ],
            ),
            opacity_track(
                rig.pelvis,
                &[
                    (36, 1.0, EasingPreset::Linear),
                    (45, 0.0, EasingPreset::Linear),
                ],
            ),
        ],
        events: vec![
            EventKey {
                frame: Frame(6),
                name: "arm_off".into(),
            },
            EventKey {
                frame: Frame(32),
                name: "thud".into(),
            },
            EventKey {
                frame: Frame(44),
                name: "died".into(),
            },
        ],
    }
}

pub fn zombie_file() -> RenFile {
    let mut doc = Document::empty();
    {
        let comp = &mut doc.compositions[doc.main];
        comp.name = "zombie".into();
        comp.size = (256, 320);
        comp.rate = FrameRate { num: 60, den: 1 };
        comp.range = (Frame(0), Frame(120));
    }

    let rig = build_rig(&mut doc);

    let mut clips = ClipMap::default();
    let walk_id = clips.insert(walk_clip(&rig));
    let eat_id = clips.insert(eat_clip(&rig));
    let fall_id = clips.insert(fall_clip(&rig));

    let machine = Machine {
        name: "zombie".into(),
        inputs: vec![
            InputDef {
                name: "eating".into(),
                kind: InputKind::Bool { default: false },
            },
            InputDef {
                name: "die".into(),
                kind: InputKind::Trigger,
            },
        ],
        layers: vec![MachineLayer {
            name: "main".into(),
            entry: 0,
            states: vec![
                State {
                    name: "walk".into(),
                    kind: StateKind::Clip {
                        clip: walk_id,
                        speed: 1.0,
                        loop_mode: LoopMode::Loop,
                    },
                    transitions: vec![Transition {
                        to: 1,
                        duration: 5.0,
                        exit_time: None,
                        conditions: vec![Condition::BoolIs {
                            input: 0,
                            value: true,
                        }],
                    }],
                },
                State {
                    name: "eat".into(),
                    kind: StateKind::Clip {
                        clip: eat_id,
                        speed: 1.0,
                        loop_mode: LoopMode::Loop,
                    },
                    transitions: vec![Transition {
                        to: 0,
                        duration: 5.0,
                        exit_time: None,
                        conditions: vec![Condition::BoolIs {
                            input: 0,
                            value: false,
                        }],
                    }],
                },
                State {
                    name: "fall".into(),
                    kind: StateKind::Clip {
                        clip: fall_id,
                        speed: 1.0,
                        loop_mode: LoopMode::Once,
                    },
                    transitions: vec![],
                },
            ],
            any_transitions: vec![Transition {
                to: 2,
                duration: 2.0,
                exit_time: None,
                conditions: vec![Condition::Triggered { input: 1 }],
            }],
        }],
        listeners: vec![],
    };

    let mut machines = MachineMap::default();
    let machine_id = machines.insert(machine);

    let mut file = RenFile::new(doc, "Zombie");
    file.clips = clips;
    file.machines = machines;
    file.clip_order = vec![walk_id, eat_id, fall_id];
    file.machine_order = vec![machine_id];
    file.start_machine = Some(machine_id);
    file.normalize();
    file.garbage_collect();
    file
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anims");
    std::fs::create_dir_all(out_dir)?;
    let out_path = format!("{out_dir}/zombie.ren");
    let ron = renamite_io_ren::save(&zombie_file())?;
    std::fs::write(&out_path, ron)?;
    println!("wrote {out_path}");
    Ok(())
}
