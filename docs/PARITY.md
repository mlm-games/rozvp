# RoZVP Parity Ledger (PvZ 0.9.9.1029)

Damage unit: **1 pea = 20**. Sim rate: **100 Hz** (1 tick = 10 ms).

Sources: **W** = wiki/strategy wiki, **D** = decomp still needed, **E** = engine choice.

## Recharge tiers
| Tier       | Seconds | Ticks | Source | Status |
|------------|---------|-------|--------|--------|
| Fast       | 7.5     | 750   | W      | ✅ |
| Slow       | 30      | 3000  | W      | ✅ |
| Very slow  | 50      | 5000  | W      | ✅ |

## Economy
| Value              | Number   | Ticks | Source | Status |
|--------------------|----------|-------|--------|--------|
| Starting sun       | 50       | —     | W      | ✅ |
| Sun value          | 25       | —     | W      | ✅ |
| Sunflower interval | 24.25 s  | 2425  | W      | ✅ |
| Sunflower first    | ~7 s     | 700   | E/D    | ⚠ |
| Sky sun interval   | —        | 425   | E      | ⚠ until Board.cpp |

## Plants (implemented)
| Plant       | Cost | Recharge   | HP   | Special                         | Source | Status |
|-------------|------|------------|------|---------------------------------|--------|--------|
| Sunflower   | 50   | Fast       | 300  | 25 sun / 24.25 s                | W      | ✅ |
| Peashooter  | 100  | Fast       | 300  | 20 dmg / **1.425 s** (143 t)    | W      | ✅ |
| Snow Pea    | 175  | Fast       | 300  | 20 dmg, chill ×0.5 (~10 s)      | W/D    | ⚠ duration TBD |
| Repeater    | 200  | Fast       | 300  | 2× peas / volley                | W      | ✅ |
| Potato Mine | 25   | Slow       | 300  | arm 15 s, contact 1800          | W      | ✅ |
| Wall-nut    | 50   | **Slow**   | 4000 | blocker                         | W      | ✅ fixed |
| Cherry Bomb | 150  | Very slow  | 300* | 1.2 s fuse, 1800, 3×3           | W      | ✅ fixed |
| Chomper     | 150  | Fast       | 300  | instant eat, 42 s chew          | W      | ✅ |

\*Cherry is single-use; HP only if eaten during fuse.

## Batch 2 plants
| Plant | Cost | Recharge | Special | Status |
|-------|------|----------|---------|--------|
| Squash | 50 | Slow | One-time 1800 crush, triggers ±1.2 tiles | ✅ |
| Threepeater | 325 | Fast | Fires own + adjacent lanes | ✅ |
| Jalapeño | 125 | Very slow | 1 s fuse, 1800 to whole row | ✅ |
| Spikeweed | 100 | Fast | 20 dmg/s per zombie standing on it; un-eatable | ✅ |
| Torchwood | 175 | Fast | Normal peas → fire (40 dmg); snow peas pass unchanged | ✅ |
| Tall-nut | 125 | Slow | 8000 HP blocker | ✅ |
| Garlic | 50 | Fast | Diverts biters to adjacent lane (RNG dir) | ✅ |
| Hypno-shroom | 75 | Slow | Eater switches sides, brawls at eat cadence | ✅ |
| Ice-shroom | 75 | Very slow | 4 s freeze-stun all → 10 s chill on thaw | ✅ approx timing |
| Doom-shroom | 125 | Very slow | 5×5 9000 dmg, 180 s crater blocks planting | ✅ |

Deferred: shrooms sleeping on Day levels (needs Coffee Bean; instant plants deferred). TODO(D) exact decomp numbers.

## Zombies (implemented)
| Zombie | Body | Armor | Total | Peas to kill (total) | Source | Status |
|--------|------|-------|-------|---------------------|--------|--------|
| Normal | 270  | 0     | 270   | 14                  | W      | ✅ |
| Flag   | 270  | 0     | 270   | 14                  | W      | ✅ same body |
| Cone   | 270  | 370   | 640   | 32                  | W      | ✅ armor |
| Bucket | 270  | 1100  | 1370  | 69                  | W      | ✅ armor |

## Adventure unlocks (current slice)
`adventure_level` 0-based: 0 Wall-nut → 1 Cherry Bomb → 2 Potato Mine →
3 Snow Pea → 4 Chomper → 5 Repeater.

## Combat timing
| Thing            | Value                    | Ticks | Source | Status |
|------------------|--------------------------|-------|--------|--------|
| Pea damage       | 20                       | —     | W      | ✅ |
| Peashooter rate  | 1.425 s                  | 143   | W      | ✅ |
| Zombie eat       | 100 dps (50 / 0.5 s)     | 50    | W      | ✅ |
| Snow slow        | ×0.5 move + eat speed    | —     | W      | ✅ |
| Snow slow time   | 10 s (current impl)      | 1000  | E/D    | ⚠ confirm in decomp |
| Zombie walk      | ~17 px/s (~4.7 s/tile)   | —     | W/E    | ⚠ |
| Cherry fuse      | 1.2 s                    | 120   | W      | ✅ |
| Cherry damage    | 1800                     | —     | W      | ✅ |
| Cherry area      | 3×3 tiles (Chebyshev ≤1) | —     | W      | ✅ |

## Waves (canonical-leaning tables)

| ID | Behavior | Value | Source | Status |
|----|----------|-------|--------|--------|
| WAVE-001 | First wave delay | 1800 ticks | D/E | ✅ |
| WAVE-002 | Inter-wave delay | 600 ticks | E | ⚠ |
| WAVE-003 | Spawn stagger | 80 ticks | E | ⚠ |
| WAVE-004 | Point: Normal | 1 | W | ✅ |
| WAVE-005 | Point: Flag | 1 | W | ✅ |
| WAVE-006 | Point: Cone | 2 | W | ✅ |
| WAVE-007 | Point: Bucket | 4 | W | ✅ |
| WAVE-008 | 1-1 waves | 4 budgets 1,1,2,3 | E/W | ✅ slice |
| WAVE-009 | 1-2.. waves | ramp + final 10 | W/E | ✅ slice |
| WAVE-010 | Flag on final wave | if ≥6 waves | W | ✅ |
| WAVE-011 | Cone unlock | adventure ≥ 1 (1-2) | E | ⚠ vs exact level |
| WAVE-012 | Bucket unlock | adventure ≥ 3 (1-4) | E | ⚠ vs exact level |
| WAVE-013 | PickZombieWaves weights | weighted roll | E | ⚠ TODO(D) Board.cpp |

Still open vs full decomp: exact per-level wave counts, exact first-wave countdown constant name, and picker weight tables.

## Stages (foundation)
| ID | Behavior | Source | Status |
|----|----------|--------|--------|
| STAGE-001 | Adventure area = level / 10 | E | ✅ |
| STAGE-002 | Area 1 = Day, Area 2+ = Night for now | W/E | ✅ |
| STAGE-003 | Night suppresses sky sun | W | ✅ |
| STAGE-004 | Night lawn tint | E | ✅ |
| STAGE-005 | Night intro advice before normal tutorial chain | E | ✅ |

## Regression checks (manual)
1. Wall-nut seed dark for **30 s** after plant (not 7.5 s).
2. Cherry seed dark for **50 s**; explodes **1.2 s** after plant.
3. Cherry kills normal + cone-in-range in one blast; 3×3 only.
4. Peashooter: **14 peas** to kill one normal zombie.
5. One zombie eats a peashooter in **~3 s** (6 bites × 0.5 s).
6. One zombie eats a wall-nut in **~40 s** (4000/100).

## Adventure flow / unlocks

| ID | Behavior | Source | Status | Notes |
|---|---|---|---|---|
| FLOW-001 | Level complete opens award screen | E | ✅ | Runtime flow implemented |
| FLOW-002 | Award unlocks seed | E/D | ✅ | Wall-nut → Cherry → Potato → Snow Pea → Chomper → Repeater; exact PvZ order TBD |
| FLOW-003 | Adventure level advances after award | E | ✅ | `adventure_level += 1` (0-based) |
| FLOW-004 | Seed chooser hides locked seeds | E | ✅ | Uses `SharedUi.unlocked_seed_names` |
| FLOW-005 | Save persistence | E | ✅ | `SaveData.{adventure_level, unlocked_seed_names}` |

## Visual states (presentational)

| ID | Behavior | Source | Status | Notes |
|---|---|---|---|---|
| VIS-001 | Cone/Bucket revert to bare body color when armor = 0 | E | ✅ | Armor strip visual |
| VIS-002 | Chilled zombies tinted blue | E | ✅ | 45% tint toward ice-blue |
| VIS-003 | Wall-nut cracks at 66% / 33% HP | W | ✅ | Two crack stages |
| VIS-004 | Cherry Bomb flashes in final 40 ticks | E | ✅ | 5-tick cadence from remaining fuse |
| VIS-005 | Potato Mine brown while dormant, orange when armed | E | ✅ | 15 s arm timer |
| VIS-006 | Chomper purple when ready, dark purple while chewing | E | ✅ | 42 s chew timer |
