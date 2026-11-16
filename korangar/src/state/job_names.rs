//! English class names for player job ids.
//!
//! Generated from Hercules `src/common/class*.h` and `conf/messages.conf`
//! (`pc_job_name`), so the party, guild, View Equipment and character select
//! windows name every class the server can send: transcendent, baby, third,
//! expanded and fourth jobs, plus the mount and outfit ids that share a class
//! name. Transcendent third jobs drop the trailing `T` Hercules uses (kRO
//! shows them as the plain class).

/// The class name, or `None` for an id Hercules does not define.
pub fn job_name(job: i32) -> Option<&'static str> {
    let name = match job {
        0 => "Novice",
        1 => "Swordsman",
        2 => "Magician",
        3 => "Archer",
        4 => "Acolyte",
        5 => "Merchant",
        6 => "Thief",
        7 => "Knight",
        8 => "Priest",
        9 => "Wizard",
        10 => "Blacksmith",
        11 => "Hunter",
        12 => "Assassin",
        13 => "Knight",
        14 => "Crusader",
        15 => "Monk",
        16 => "Sage",
        17 => "Rogue",
        18 => "Alchemist",
        19 => "Bard",
        20 => "Dancer",
        21 => "Crusader",
        22 => "Wedding",
        23 => "Super Novice",
        24 => "Gunslinger",
        25 => "Ninja",
        26 => "Christmas",
        27 => "Summer",
        4001 => "High Novice",
        4002 => "High Swordsman",
        4003 => "High Magician",
        4004 => "High Archer",
        4005 => "High Acolyte",
        4006 => "High Merchant",
        4007 => "High Thief",
        4008 => "Lord Knight",
        4009 => "High Priest",
        4010 => "High Wizard",
        4011 => "Whitesmith",
        4012 => "Sniper",
        4013 => "Assassin Cross",
        4014 => "Lord Knight",
        4015 => "Paladin",
        4016 => "Champion",
        4017 => "Professor",
        4018 => "Stalker",
        4019 => "Creator",
        4020 => "Clown",
        4021 => "Gypsy",
        4022 => "Paladin",
        4023 => "Baby Novice",
        4024 => "Baby Swordsman",
        4025 => "Baby Magician",
        4026 => "Baby Archer",
        4027 => "Baby Acolyte",
        4028 => "Baby Merchant",
        4029 => "Baby Thief",
        4030 => "Baby Knight",
        4031 => "Baby Priest",
        4032 => "Baby Wizard",
        4033 => "Baby Blacksmith",
        4034 => "Baby Hunter",
        4035 => "Baby Assassin",
        4036 => "Baby Knight",
        4037 => "Baby Crusader",
        4038 => "Baby Monk",
        4039 => "Baby Sage",
        4040 => "Baby Rogue",
        4041 => "Baby Alchemist",
        4042 => "Baby Bard",
        4043 => "Baby Dancer",
        4044 => "Baby Crusader",
        4045 => "Super Baby",
        4046 => "Taekwon",
        4047 => "Star Gladiator",
        4048 => "Star Gladiator",
        4049 => "Soul Linker",
        4050 => "Gangsi",
        4051 => "Death Knight",
        4052 => "Dark Collector",
        4054 => "Rune Knight",
        4055 => "Warlock",
        4056 => "Ranger",
        4057 => "Arch Bishop",
        4058 => "Mechanic",
        4059 => "Guillotine Cross",
        4060 => "Rune Knight",
        4061 => "Warlock",
        4062 => "Ranger",
        4063 => "Arch Bishop",
        4064 => "Mechanic",
        4065 => "Guillotine Cross",
        4066 => "Royal Guard",
        4067 => "Sorcerer",
        4068 => "Minstrel",
        4069 => "Wanderer",
        4070 => "Sura",
        4071 => "Genetic",
        4072 => "Shadow Chaser",
        4073 => "Royal Guard",
        4074 => "Sorcerer",
        4075 => "Minstrel",
        4076 => "Wanderer",
        4077 => "Sura",
        4078 => "Genetic",
        4079 => "Shadow Chaser",
        4080 => "Rune Knight",
        4081 => "Rune Knight",
        4082 => "Royal Guard",
        4083 => "Royal Guard",
        4084 => "Ranger",
        4085 => "Ranger",
        4086 => "Mechanic",
        4087 => "Mechanic",
        4088 => "Rune Knight",
        4089 => "Rune Knight",
        4090 => "Rune Knight",
        4091 => "Rune Knight",
        4092 => "Rune Knight",
        4093 => "Rune Knight",
        4094 => "Rune Knight",
        4095 => "Rune Knight",
        4096 => "Baby Rune Knight",
        4097 => "Baby Warlock",
        4098 => "Baby Ranger",
        4099 => "Baby Arch Bishop",
        4100 => "Baby Mechanic",
        4101 => "Baby Guillotine Cross",
        4102 => "Baby Royal Guard",
        4103 => "Baby Sorcerer",
        4104 => "Baby Minstrel",
        4105 => "Baby Wanderer",
        4106 => "Baby Sura",
        4107 => "Baby Genetic",
        4108 => "Baby Shadow Chaser",
        4109 => "Baby Rune Knight",
        4110 => "Baby Royal Guard",
        4111 => "Baby Ranger",
        4112 => "Baby Mechanic",
        4190 => "Expanded Super Novice",
        4191 => "Expanded Super Baby",
        4211 => "Kagerou",
        4212 => "Oboro",
        4215 => "Rebellion",
        4218 => "Summoner",
        4220 => "Baby Summoner",
        4222 => "Baby Ninja",
        4223 => "Baby Kagerou",
        4224 => "Baby Oboro",
        4225 => "Baby Taekwon",
        4226 => "Baby Star Gladiator",
        4227 => "Baby Soul Linker",
        4228 => "Baby Gunslinger",
        4229 => "Baby Rebellion",
        4238 => "Baby Star Gladiator",
        4239 => "Star Emperor",
        4240 => "Soul Reaper",
        4241 => "Baby Star Emperor",
        4242 => "Baby Soul Reaper",
        4252 => "Dragon Knight",
        4253 => "Meister",
        4254 => "Shadow Cross",
        4255 => "Arch Mage",
        4256 => "Cardinal",
        4257 => "Windhawk",
        4258 => "Imperial Guard",
        4259 => "Biolo",
        4260 => "Abyss Chaser",
        4261 => "Elemental Master",
        4262 => "Inquisitor",
        4263 => "Troubadour",
        4264 => "Trouvere",
        4278 => "Windhawk",
        4279 => "Meister",
        4280 => "Dragon Knight",
        4281 => "Imperial Guard",
        4302 => "Sky Emperor",
        4303 => "Soul Ascetic",
        4304 => "Shinkiro",
        4305 => "Shiranui",
        4306 => "Night Watch",
        4307 => "Hyper Novice",
        4308 => "Spirit Handler",
        4316 => "Sky Emperor",
        _ => return None,
    };
    Some(name)
}

/// The class name, or `Job <id>` for an unknown id.
pub fn job_label(job: i16) -> String {
    match job_name(i32::from(job)) {
        Some(name) => name.to_owned(),
        None => format!("Job {job}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{job_label, job_name};

    #[test]
    fn first_second_and_transcendent_classes() {
        assert_eq!(job_name(0), Some("Novice"));
        assert_eq!(job_name(1), Some("Swordsman"));
        assert_eq!(job_name(7), Some("Knight"));
        assert_eq!(job_name(13), Some("Knight"), "Peco Knight shares the class name");
        assert_eq!(job_name(4008), Some("Lord Knight"));
        assert_eq!(job_name(4014), Some("Lord Knight"));
        assert_eq!(job_name(4001), Some("High Novice"));
        assert_eq!(job_name(4020), Some("Clown"));
        assert_eq!(job_name(4021), Some("Gypsy"));
    }

    #[test]
    fn ids_26_to_30_are_not_high_classes() {
        // The old party table named 26..=30 Novice High .. Acolyte High; those
        // are 4001..=4005. 26 and 27 are the Christmas and Summer outfits.
        assert_eq!(job_name(26), Some("Christmas"));
        assert_eq!(job_name(27), Some("Summer"));
        assert_eq!(job_name(28), None);
        assert_eq!(job_name(30), None);
    }

    #[test]
    fn third_expanded_and_fourth_classes() {
        assert_eq!(job_name(4054), Some("Rune Knight"));
        assert_eq!(job_name(4060), Some("Rune Knight"), "transcendent drops the T");
        assert_eq!(job_name(4080), Some("Rune Knight"), "dragon mount");
        assert_eq!(job_name(4075), Some("Minstrel"));
        assert_eq!(job_name(4096), Some("Baby Rune Knight"));
        assert_eq!(job_name(4190), Some("Expanded Super Novice"));
        assert_eq!(job_name(4211), Some("Kagerou"));
        assert_eq!(job_name(4215), Some("Rebellion"));
        assert_eq!(job_name(4218), Some("Summoner"));
        assert_eq!(job_name(4239), Some("Star Emperor"));
        assert_eq!(job_name(4252), Some("Dragon Knight"));
        assert_eq!(job_name(4280), Some("Dragon Knight"));
        assert_eq!(job_name(4262), Some("Inquisitor"));
        assert_eq!(job_name(4307), Some("Hyper Novice"));
        assert_eq!(job_name(4308), Some("Spirit Handler"));
        assert_eq!(job_name(4316), Some("Sky Emperor"));
    }

    #[test]
    fn unknown_ids_fall_back_to_the_number() {
        assert_eq!(job_name(4053), None);
        assert_eq!(job_name(4113), None);
        assert_eq!(job_label(999), "Job 999");
        assert_eq!(job_label(-1), "Job -1");
        assert_eq!(job_label(4255), "Arch Mage");
    }
}
