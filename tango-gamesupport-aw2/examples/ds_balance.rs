//! Balance report: every AW2 unit's stats and damage rows as Advance Wars 2
//! has them and as the Dual Strike pack gives them, side by side, with
//! every attacker/defender pair whose base damage changes.
//!
//! Usage: TANGOAW2_DS_ROM=<Dual Strike ROM or pack> ds_balance <aw2 rom>

use tango_gamesupport_aw2::ds_units::{ds_fields, AW2_TYPES, UNITS};

const NAMES: [&str; 26] = [
    "-",
    "Infantry",
    "Mech",
    "Md Tank",
    "Megatank",
    "Tank",
    "Recon",
    "APC",
    "Neotank",
    "Piperunner",
    "Artillery",
    "Rockets",
    "Stealth",
    "Black Bomb",
    "Anti-Air",
    "Missiles",
    "Fighter",
    "Bomber",
    "Black Boat",
    "B Copter",
    "T Copter",
    "Battleship",
    "Cruiser",
    "Lander",
    "Sub",
    "dived Sub",
];

fn main() {
    let rom = std::fs::read(std::env::args().nth(1).expect("aw2 rom")).unwrap();
    let pack = tango_gamesupport_aw2::ds_pack::pack().expect("TANGOAW2_DS_ROM: a Dual Strike ROM or pack");
    let aw2 = |id: u32, off: u32, len: usize| -> Vec<u8> {
        let a = (UNITS - 0x0800_0000 + 0x5C * id + off) as usize;
        rom[a..a + len].to_vec()
    };
    println!("unit          | cost        | move  | ammo  | vision | range       | fuel");
    let mut damage_changes = Vec::new();
    for id in AW2_TYPES {
        let ds = pack
            .overlay_at(0, 0x022A_D560, 0x022A_D560 + 0x47A58 + 0x6C * id, 0x6C)
            .unwrap();
        let f = ds_fields(ds);
        let old_cost = u16::from_le_bytes(aw2(id, 0x06, 2).try_into().unwrap()) as u32 * 10;
        let new_cost = u16::from_le_bytes(f[0].1.clone().try_into().unwrap()) as u32 * 10;
        let o1 = aw2(id, 0x0A, 3);
        let o2 = aw2(id, 0x0E, 3);
        let mark = |a: u32, b: u32| if a == b { format!("{a}") } else { format!("{a}->{b}") };
        println!(
            "{:13} | {:11} | {:5} | {:5} | {:6} | {:11} | {}",
            NAMES[id as usize],
            mark(old_cost, new_cost),
            mark(o1[0] as u32, f[1].1[0] as u32),
            mark(o1[1] as u32, f[1].1[1] as u32),
            mark(o1[2] as u32, f[1].1[2] as u32),
            if (o2[0], o2[1]) == (f[2].1[0], f[2].1[1]) {
                format!("{}-{}", o2[0], o2[1])
            } else {
                format!("{}-{}->{}-{}", o2[0], o2[1], f[2].1[0], f[2].1[1])
            },
            mark(o2[2] as u32, f[2].1[2] as u32),
        );
        for (row, off, name) in [(3, 0x1E, "primary"), (4, 0x38, "secondary")] {
            let old = aw2(id, off, 26);
            for def in 1..26 {
                // Only defenders AW2 has (new units come with their own stage).
                if ![4, 9, 12, 13, 18].contains(&def) && old[def] != f[row].1[def] {
                    damage_changes.push(format!(
                        "  {:10} {:9} vs {:10}: {:3} -> {:3}",
                        NAMES[id as usize], name, NAMES[def], old[def], f[row].1[def]
                    ));
                }
            }
        }
    }
    println!("\ndamage changes between existing units ({}):", damage_changes.len());
    for c in damage_changes {
        println!("{c}");
    }
}
