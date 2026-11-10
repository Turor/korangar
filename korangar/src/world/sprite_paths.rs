//! Player body, head, and headgear virtual paths.
//!
//! These are Unicode paths, the same strings the GRF and the release CDN use.
//! `묘족` is not a body folder. Doram bodies live under `도람족`.

use ragnarok_packets::{JobId, Sex};

const MALE_HAIR_LOOKUP: &[usize] = &[2, 2, 1, 7, 5, 4, 3, 6, 8, 9, 10, 12, 11];
const FEMALE_HAIR_LOOKUP: &[usize] = &[2, 2, 4, 7, 1, 5, 3, 6, 12, 10, 9, 11, 8];
/// roBrowser `HairIndexTable` Doram rows. The index is the style id.
const DORAM_HAIR_LOOKUP: &[usize] = &[0, 1, 2, 3, 4, 5, 6];

pub(crate) fn is_doram_job(job_id: JobId) -> bool {
    matches!(job_id.0, 4217..=4220 | 4308 | 4315)
}

fn sex_folder(sex: Sex) -> &'static str {
    if sex == Sex::Female { "여" } else { "남" }
}

/// Head sprite file id. roBrowser `UpdateHead` turns style `0` into `1` and
/// `getHeadPath` uses that style index for the file. `HairIndexTable` is only
/// the palette id ([`hair_palette_id`]).
pub(crate) fn head_style_id(head: Option<usize>) -> usize {
    match head {
        Some(0) | None => 1,
        Some(head) => head,
    }
}

/// Palette file id for a head (`HairIndexTable`, roBrowser `getHeadPalPath`).
pub(crate) fn hair_palette_id(sex: Sex, doram: bool, head: Option<usize>) -> usize {
    let Some(head) = head else {
        return 1;
    };
    let table = match (doram, sex) {
        (true, _) => DORAM_HAIR_LOOKUP,
        (false, Sex::Female) => FEMALE_HAIR_LOOKUP,
        (false, _) => MALE_HAIR_LOOKUP,
    };
    table.get(head).copied().unwrap_or(head)
}

#[allow(clippy::invisible_characters)]
pub(crate) fn job_sprite_name(job_id: JobId) -> &'static str {
    match job_id.0 {
        4218 | 4220 => "summoner",       // DO_SUMMONER / DO_SUMMONER_B
        4219 => "cart_summoner",         // DO_CART_SUMMONER1 (file exists; JobNameTable has no row)
        4308 | 4315 => "spirit_handler", // SPIRIT_HANDLER and the riding id
        0 => "초보자",                   // NOVICE
        1 => "검사",                     // SWORDMAN
        2 => "마법사",                   // MAGICIAN
        3 => "궁수",                     // ARCHER
        4 => "성직자",                   // ACOLYTE
        5 => "상인",                     // MERCHANT
        6 => "도둑",                     // THIEF
        7 => "기사",                     // KNIGHT
        8 => "성투사",                   // PRIEST
        9 => "위저드",                   // WIZARD
        10 => "제철공",                  // BLACKSMITH
        11 => "헌터",                    // HUNTER
        12 => "어세신",                  // ASSASSIN
        13 => "페코페코_기사",           // KNIGHT2
        14 => "크루세이더",              // CRUSADER
        15 => "몽크",                    // MONK
        16 => "세이지",                  // SAGE
        17 => "로그",                    // ROGUE
        18 => "연금술사",                // ALCHEMIST
        19 => "바드",                    // BARD
        20 => "무희",                    // DANCER
        23 => "슈퍼노비스",              // SUPERNOVICE
        24 => "건너",                    // GUNSLINGER
        25 => "닌자",                    // NINJA
        4001 => "초보자",                // NOVICE_H
        4002 => "검사",                  // SWORDMAN_H
        4003 => "마법사",                // MAGICIAN_H
        4004 => "궁수",                  // ARCHER_H
        4005 => "성직자",                // ACOLYTE_H
        4006 => "상인",                  // MERCHANT_H
        4007 => "도둑",                  // THIEF_H
        4008 => "로드나이트",            // KNIGHT_H
        4009 => "하이프리",              // PRIEST_H
        4010 => "하이위저드",            // WIZARD_H
        4011 => "화이트스미스",          // BLACKSMITH_H
        4012 => "스나이퍼",              // HUNTER_H
        4013 => "어쌔신크로스",          // ASSASSIN_H
        4014 => "엔대운",                // CHICKEN_H
        4015 => "팔라딘",                // CRUSADER_H
        4016 => "챔피온",                // MONK_H
        4017 => "프로페서",              // SAGE_H
        4018 => "스토커",                // ROGUE_H
        4019 => "크리에이터",            // ALCHEMIST_H
        4020 => "클라운",                // BARD_H
        4021 => "집시",                  // DANCER_H
        4023 => "초보자",                // NOVICE_B
        4024 => "검사",                  // SWORDMAN_B
        4025 => "마법사",                // MAGICIAN_B
        4026 => "궁수",                  // ARCHER_B
        4027 => "성직자",                // ACOLYTE_B
        4028 => "상인",                  // MERCHANT_B
        4029 => "도둑",                  // THIEF_B
        4030 => "기사",                  // KNIGHT_B
        4031 => "성투사",                // PRIEST_B
        4032 => "위저드",                // WIZARD_B
        4033 => "제철공",                // BLACKSMITH_B
        4034 => "헌터",                  // HUNTER_B
        4035 => "어세신",                // ASSASSIN_B
        4037 => "크루세이더",            // CRUSADER_B
        4038 => "몽크",                  // MONK_B
        4039 => "세이지",                // SAGE_B
        4040 => "로그",                  // ROGUE_B
        4041 => "연금술사",              // ALCHEMIST_B
        4042 => "바드",                  // BARD_B
        4043 => "무희",                  // DANCER_B
        4045 => "슈퍼노비스",            // SUPERNOVICE_B
        4054 => "룬나이트",              // RUNE_KNIGHT
        4055 => "워록",                  // WARLOCK
        4056 => "레인져",                // RANGER
        4057 => "아크비숍",              // ARCH_BISHOP
        4058 => "미케닉",                // MECHANIC
        4059 => "길로틴크로스",          // GUILLOTINE_CROSS
        4066 => "가드",                  // ROYAL_GUARD
        4067 => "소서러",                // SORCERER
        4068 => "민스트럴",              // MINSTREL
        4069 => "원더러",                // WANDERER
        4070 => "슈라",                  // SURA
        4071 => "제네릭",                // GENETIC
        4072 => "쉐도우체이서",          // SHADOW_CHASER
        4060 => "룬나이트",              // RUNE_KNIGHT_H
        4061 => "워록",                  // WARLOCK_H
        4062 => "레인져",                // RANGER_H
        4063 => "아크비숍",              // ARCH_BISHOP_H
        4064 => "미케닉",                // MECHANIC_H
        4065 => "길로틴크로스",          // GUILLOTINE_CROSS_H
        4073 => "가드",                  // ROYAL_GUARD_H
        4074 => "소서러",                // SORCERER_H
        4075 => "민스트럴",              // MINSTREL_H
        4076 => "원더러",                // WANDERER_H
        4077 => "슈라",                  // SURA_H
        4078 => "제네릭",                // GENETIC_H
        4079 => "쉐도우체이서",          // SHADOW_CHASER_H
        4096 => "룬나이트",              // RUNE_KNIGHT_B
        4097 => "워록",                  // WARLOCK_B
        4098 => "레인져",                // RANGER_B
        4099 => "아크비숍",              // ARCHBISHOP_B
        4100 => "미케닉",                // MECHANIC_B
        4101 => "길로틴크로스",          // GUILLOTINE_CROSS_B
        4102 => "가드",                  // ROYAL_GUARD_B
        4103 => "소서러",                // SORCERER_B
        4104 => "민스트럴",              // MINSTREL_B
        4105 => "원더러",                // WANDERER_B
        4106 => "슈라",                  // SURA_B
        4107 => "제네릭",                // GENETIC_B
        4108 => "쉐도우체이서",          // SHADOW_CHASER_B
        4046 => "태권소년",              // TAEKWON
        4047 => "권성",                  // STAR
        4049 => "소울링커",              // LINKER
        4190 => "슈퍼노비스",            // SUPERNOVICE2
        4191 => "슈퍼노비스",            // SUPERNOVICE2_B
        4211 => "KAGEROU",               // KAGEROU
        4212 => "OBORO",                 // OBORO
        4215 => "REBELLION",             // REBELLION
        4222 => "닌자",                  // NINJA_B
        4223 => "KAGEROU",               // KAGEROU_B
        4224 => "OBORO",                 // OBORO_B
        4225 => "태권소년",              // TAEKWON_B
        4226 => "권성",                  // STAR_B
        4227 => "소울링커",              // LINKER_B
        4228 => "건너",                  // GUNSLINGER_B
        4229 => "REBELLION",             // REBELLION_B
        4239 => "성제",                  // STAR EMPEROR
        4240 => "소울리퍼",              // SOUL REAPER
        4241 => "성제",                  // STAR_EMPEROR_B
        4242 => "소울리퍼",              // SOUL_REAPER_B
        4252 => "DRAGON_KNIGHT",         // DRAGON KNIGHT
        4253 => "MEISTER",               // MEISTER
        4254 => "SHADOW_CROSS",          // SHADOW CROSS
        4255 => "ARCH_MAGE",             // ARCH MAGE
        4256 => "CARDINAL",              // CARDINAL
        4257 => "WINDHAWK",              // WINDHAWK
        4258 => "IMPERIAL_GUARD",        // IMPERIAL GUARD
        4259 => "BIOLO",                 // BIOLO
        4260 => "ABYSS_CHASER",          // ABYSS CHASER
        4261 => "ELEMETAL_MASTER",       // ELEMENTAL MASTER
        4262 => "INQUISITOR",            // INQUISITOR
        4263 => "TROUBADOUR",            // TROUBADOUR
        4264 => "TROUVERE",              // TROUVERE
        4302 => "SKY_EMPEROR",           // SKY EMPEROR
        4303 => "SOUL_ASCETIC",          // SOUL ASCETIC
        4304 => "SHINKIRO",              // SHINKIRO
        4305 => "SHIRANUI",              // SHIRANUI
        4306 => "NIGHT_WATCH",           // NIGHT WATCH
        4307 => "HYPER_NOVICE",          // HYPER NOVICE
        _ => "초보자",                   // NOVICE,
    }
}

/// Body then head. Sprite and action loaders append `.spr` / `.act` under
/// `data\sprite`.
pub(crate) fn player_part_files(job_id: JobId, sex: Sex, head: Option<usize>) -> Vec<String> {
    let sex_name = sex_folder(sex);
    let doram = is_doram_job(job_id);
    let tribe = if doram { "도람족" } else { "인간족" };
    let head_id = head_style_id(head);
    let job = job_sprite_name(job_id);
    vec![
        format!("{tribe}\\몸통\\{sex_name}\\{job}_{sex_name}"),
        format!("{tribe}\\머리통\\{sex_name}\\{head_id}_{sex_name}"),
    ]
}

/// roBrowser `getHatPath`: `악세사리/{sex}/{sex}{HatTable name}`.
/// Hat names already include their leading underscore (`_고글`).
/// The client has no HatTable loader yet, so nothing calls this outside tests.
#[allow(dead_code)]
pub(crate) fn headgear_sprite_path(sex: Sex, accessory_name: &str) -> String {
    let sex_name = sex_folder(sex);
    format!("악세사리\\{sex_name}\\{sex_name}{accessory_name}")
}

/// Hercules `JOB_NOVICE`.
pub const JOB_NOVICE: u16 = 0;
/// Hercules `JOB_SUMMONER`. The only other job `char_make_new_char_sql`
/// accepts.
pub const JOB_SUMMONER: u16 = 4218;

/// Novice or Summoner files for the creation preview.
pub(crate) fn creation_part_files(doram: bool, female: bool, style: u16) -> Vec<String> {
    let sex = if female { Sex::Female } else { Sex::Male };
    let job_id = JobId(if doram { JOB_SUMMONER } else { JOB_NOVICE });
    player_part_files(job_id, sex, Some(usize::from(style)))
}

/// GRF path of the hair palette. `palette_id` is [`hair_palette_id`], not the
/// style index.
pub(crate) fn hair_palette_grf_path(doram: bool, female: bool, palette_id: usize, color: u16) -> String {
    let sex = if female { "여" } else { "남" };
    match doram {
        true => format!("data\\palette\\도람족\\머리\\머리{palette_id}_{sex}_{color}.pal"),
        false => format!("data\\palette\\머리\\머리{palette_id}_{sex}_{color}.pal"),
    }
}

/// Path under `data\\texture\\` for a CharCreate v4 hair thumbnail.
pub(crate) fn hair_thumb_grf_path(doram: bool, female: bool, style: u16) -> String {
    let name = match (doram, female) {
        (true, true) => format!("img_hairstyle_doramgirl{style:02}"),
        (true, false) => format!("img_hairstyle_doramboy{style:02}"),
        (false, true) => format!("img_hairstyle_girl{style:02}"),
        (false, false) => format!("img_hairstyle{style:02}"),
    };
    format!("유저인터페이스\\make_character_ver2\\{name}.bmp")
}

/// Path under `data\\texture\\` for a hair-color swatch. `color` is 0-based.
pub(crate) fn hair_color_thumb_grf_path(color: u16) -> String {
    format!("유저인터페이스\\make_character_ver2\\color{:02}_off.bmp", color + 1)
}

#[cfg(test)]
mod tests {
    use korangar_loaders::remote::canonical_path;

    use super::*;
    use crate::world::inventory_icon_texture_path;

    fn sprite_key(path: &str) -> String {
        canonical_path(&format!("data\\sprite\\{path}.spr")).unwrap()
    }

    #[test]
    fn body_head_headgear_item_icon_and_doram_paths_match_the_release_keys() {
        let novice = player_part_files(JobId(0), Sex::Male, Some(0));
        assert_eq!(novice[0], "인간족\\몸통\\남\\초보자_남");
        assert_eq!(novice[1], "인간족\\머리통\\남\\1_남");
        assert_eq!(sprite_key(&novice[0]), "data/sprite/인간족/몸통/남/초보자_남.spr");
        assert_eq!(sprite_key(&novice[1]), "data/sprite/인간족/머리통/남/1_남.spr");

        let hat = headgear_sprite_path(Sex::Male, "_고글");
        assert_eq!(hat, "악세사리\\남\\남_고글");
        assert_eq!(sprite_key(&hat), "data/sprite/악세사리/남/남_고글.spr");
        assert_eq!(
            sprite_key(&headgear_sprite_path(Sex::Female, "_리본")),
            "data/sprite/악세사리/여/여_리본.spr"
        );

        let icon = inventory_icon_texture_path("빨간포션").unwrap();
        assert_eq!(icon, "유저인터페이스\\item\\빨간포션.bmp");
        assert_eq!(
            canonical_path(&format!("data\\texture\\{icon}")).as_deref(),
            Some("data/texture/유저인터페이스/item/빨간포션.bmp")
        );

        let summoner = player_part_files(JobId(4218), Sex::Female, Some(1));
        assert_eq!(summoner[0], "도람족\\몸통\\여\\summoner_여");
        assert_eq!(summoner[1], "도람족\\머리통\\여\\1_여");
        assert!(!summoner[0].contains("묘족"));
        assert!(!summoner[0].contains("인간족"));
        assert_eq!(sprite_key(&summoner[0]), "data/sprite/도람족/몸통/여/summoner_여.spr");
        assert_eq!(
            player_part_files(JobId(4220), Sex::Female, None)[0],
            "도람족\\몸통\\여\\summoner_여"
        );
        assert_eq!(
            player_part_files(JobId(4219), Sex::Female, None)[0],
            "도람족\\몸통\\여\\cart_summoner_여"
        );
        assert_eq!(
            player_part_files(JobId(4308), Sex::Male, None)[0],
            "도람족\\몸통\\남\\spirit_handler_남"
        );
        assert_eq!(
            player_part_files(JobId(4315), Sex::Male, None)[0],
            "도람족\\몸통\\남\\spirit_handler_남"
        );
        // 4217 has no JobNameTable row. The folder is still Doram, not 묘족.
        let tribe = player_part_files(JobId(4217), Sex::Female, None);
        assert!(tribe[0].starts_with("도람족\\몸통\\"));
        assert!(!tribe[0].contains("묘족"));
    }

    #[test]
    fn human_head_file_is_the_style_index_and_palette_uses_the_table() {
        assert_eq!(creation_part_files(false, false, 1)[1], "인간족\\머리통\\남\\1_남");
        assert_eq!(creation_part_files(false, true, 2)[1], "인간족\\머리통\\여\\2_여");
        assert_eq!(
            player_part_files(JobId(JOB_NOVICE), Sex::Male, Some(0))[1],
            "인간족\\머리통\\남\\1_남"
        );
        let palette_id = hair_palette_id(Sex::Male, false, Some(1));
        assert_eq!(palette_id, 2);
        assert_eq!(
            hair_palette_grf_path(false, false, palette_id, 3),
            "data\\palette\\머리\\머리2_남_3.pal"
        );
        assert_eq!(hair_palette_id(Sex::Female, false, Some(2)), 4);
    }

    #[test]
    fn doram_creation_preview_is_the_summoner_body() {
        let files = creation_part_files(true, true, 3);
        assert_eq!(files[0], "도람족\\몸통\\여\\summoner_여");
        assert_eq!(files[1], "도람족\\머리통\\여\\3_여");
        assert_eq!(hair_palette_id(Sex::Female, true, Some(3)), 3);
        assert_eq!(
            hair_palette_grf_path(true, true, 3, 1),
            "data\\palette\\도람족\\머리\\머리3_여_1.pal"
        );
    }

    #[test]
    fn hair_thumbnails_follow_the_grf() {
        assert_eq!(
            hair_thumb_grf_path(false, false, 2),
            "유저인터페이스\\make_character_ver2\\img_hairstyle02.bmp"
        );
        assert!(hair_thumb_grf_path(false, true, 2).ends_with("img_hairstyle_girl02.bmp"));
        assert!(hair_thumb_grf_path(true, false, 2).ends_with("img_hairstyle_doramboy02.bmp"));
        assert!(hair_thumb_grf_path(true, true, 6).ends_with("img_hairstyle_doramgirl06.bmp"));
        assert_eq!(
            hair_color_thumb_grf_path(0),
            "유저인터페이스\\make_character_ver2\\color01_off.bmp"
        );
    }
}
