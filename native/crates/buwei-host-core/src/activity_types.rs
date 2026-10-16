//! Shared activity vocabulary. Templates supply editable suggestions, never facts or permissions.
use crate::Result;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
pub struct ActivityType {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub aliases: &'static [&'static str],
    pub title: &'static str,
    pub capacity: u8,
    pub description: &'static str,
    pub agenda: &'static [&'static str],
    pub preparation: &'static [&'static str],
}

pub const ALL: &[ActivityType] = &[
    ActivityType {
        id: "badminton",
        name: "羽毛球",
        category: "运动健身",
        aliases: &["羽毛球", "打羽毛球", "badminton"],
        title: "羽毛球活动",
        capacity: 6,
        description: "请提前到场，自备球拍。场地、用球和费用由组织者确认。",
        agenda: &["签到与热身", "按人数分组轮换", "放松与收拾场地"],
        preparation: &["确认场地、球拍和用球", "说明分组方式及费用安排"],
    },
    ActivityType {
        id: "boardgame",
        name: "桌游",
        category: "文娱休闲",
        aliases: &["桌游", "桌面游戏", "boardgame", "board game"],
        title: "桌游活动",
        capacity: 6,
        description: "请在报名时填写同行人数。游戏、规则和新手安排由组织者说明。",
        agenda: &["签到与规则介绍", "分桌体验与换组", "收纳游戏与交流"],
        preparation: &["确认游戏及每桌人数", "准备新手规则说明和配件"],
    },
    ActivityType {
        id: "reading",
        name: "读书会",
        category: "学习交流",
        aliases: &["读书会", "读书", "阅读分享", "reading", "book club"],
        title: "读书会",
        capacity: 12,
        description: "请准备一句想分享的阅读感受。书目、阅读范围和交流方式由组织者确认。",
        agenda: &[
            "介绍书目与交流主题",
            "轮流分享与讨论",
            "整理问题与下一次阅读建议",
        ],
        preparation: &["确认书目与阅读范围", "准备讨论问题和笔记"],
    },
    ActivityType {
        id: "running",
        name: "跑步",
        category: "运动健身",
        aliases: &["跑步", "夜跑", "晨跑", "running", "run"],
        title: "一起跑步",
        capacity: 8,
        description: "集合点、路线、距离和配速由组织者确认。请按自身情况参加，留意天气。",
        agenda: &["集合与热身", "按约定路线及配速跑步", "拉伸与确认成员返回"],
        preparation: &["确认路线、距离、配速及集合点", "提醒携带饮水并关注天气"],
    },
    ActivityType {
        id: "hiking",
        name: "徒步",
        category: "户外出行",
        aliases: &["徒步", "登山", "爬山", "hiking", "hike"],
        title: "周末徒步",
        capacity: 10,
        description: "路线、难度、集合与返程安排由组织者确认。请按自身情况参加，留意天气和装备。",
        agenda: &["集合与路线说明", "沿约定路线徒步及休息", "清点成员与返程"],
        preparation: &["确认距离、难度、天气及返程安排", "说明饮水、鞋服和必要装备"],
    },
    ActivityType {
        id: "cycling",
        name: "骑行",
        category: "户外出行",
        aliases: &["骑行", "骑车", "cycling", "bike ride"],
        title: "社群骑行",
        capacity: 8,
        description: "集合点、路线、距离和速度由组织者确认。自备合适车辆与头盔，遵守交通规则。",
        agenda: &["集合与车辆检查", "按路线骑行及途中休息", "清点成员与返程"],
        preparation: &["确认路线、速度及补给点", "提醒检查车辆、照明与头盔"],
    },
    ActivityType {
        id: "basketball",
        name: "篮球",
        category: "运动健身",
        aliases: &["篮球", "打篮球", "basketball"],
        title: "篮球活动",
        capacity: 10,
        description: "场地、分组和赛制由组织者确认。请提前热身，自备饮水。",
        agenda: &["签到与热身", "分组比赛及轮换", "放松与收拾场地"],
        preparation: &["确认场地、篮球和分组", "说明赛制及轮换方式"],
    },
    ActivityType {
        id: "football",
        name: "足球",
        category: "运动健身",
        aliases: &["足球", "踢球", "football", "soccer"],
        title: "足球活动",
        capacity: 14,
        description: "场地、人数和赛制由组织者确认。请准备合适鞋服，提前热身。",
        agenda: &["签到与热身", "分组比赛及轮换", "放松与收拾场地"],
        preparation: &["确认场地、用球及每队人数", "说明鞋服要求与轮换方式"],
    },
    ActivityType {
        id: "fitness",
        name: "健身瑜伽",
        category: "运动健身",
        aliases: &["健身瑜伽", "健身", "瑜伽", "普拉提", "fitness", "yoga"],
        title: "健身交流活动",
        capacity: 8,
        description: "训练内容、强度、场地和带领者由组织者确认。请按自身情况参加，自备所需装备。",
        agenda: &["说明内容与热身", "按已确认的安排练习", "放松与交流"],
        preparation: &["确认内容、强度和带领者", "说明鞋服、瑜伽垫等装备要求"],
    },
    ActivityType {
        id: "movie",
        name: "观影",
        category: "文娱休闲",
        aliases: &["观影", "电影", "看电影", "movie", "film"],
        title: "一起观影",
        capacity: 10,
        description: "片名、放映地点、观看方式和费用由组织者确认；活动报名不代表已购票或订座。",
        agenda: &["集合与观影说明", "观看已确认影片", "观后交流"],
        preparation: &["确认片名、时长与观看条件", "另行核实购票或场地安排"],
    },
    ActivityType {
        id: "music",
        name: "音乐",
        category: "文娱休闲",
        aliases: &["音乐", "合唱", "乐器", "唱歌", "music", "singing"],
        title: "音乐交流活动",
        capacity: 10,
        description: "曲目、形式、场地和设备由组织者确认。请说明是否适合新手及所需乐器。",
        agenda: &["签到与曲目介绍", "分组练习或音乐分享", "交流与整理设备"],
        preparation: &["确认曲目、形式和设备", "说明乐器及新手参与方式"],
    },
    ActivityType {
        id: "photography",
        name: "摄影",
        category: "户外出行",
        aliases: &["摄影", "拍照", "外拍", "photography", "photo walk"],
        title: "摄影交流活动",
        capacity: 8,
        description: "拍摄主题、路线和器材要求由组织者确认。拍摄与分享他人肖像前请征得同意。",
        agenda: &["集合与拍摄主题说明", "按约定范围拍摄", "作品交流与返程"],
        preparation: &["确认拍摄地点、天气与器材", "说明肖像及作品分享约定"],
    },
    ActivityType {
        id: "dining",
        name: "聚餐",
        category: "聚会生活",
        aliases: &["聚餐", "聚会", "吃饭", "餐叙", "dining", "dinner"],
        title: "社群聚餐",
        capacity: 8,
        description: "地点、预算、饮食安排及费用方式由组织者确认；报名不代表餐厅已订位。",
        agenda: &["集合与介绍", "用餐及交流", "核对费用与结束安排"],
        preparation: &["确认餐厅、预算与订位情况", "说明饮食需求的反馈方式"],
    },
    ActivityType {
        id: "workshop",
        name: "工作坊",
        category: "学习交流",
        aliases: &["工作坊", "手作", "手工", "绘画", "workshop", "craft"],
        title: "主题工作坊",
        capacity: 12,
        description: "主题、适合人群、材料和费用由组织者确认。请说明是否需要自备工具。",
        agenda: &["介绍主题与操作方法", "分组实践及交流", "成果分享与整理材料"],
        preparation: &["确认主题、带领者与材料数量", "说明自备工具及费用安排"],
    },
    ActivityType {
        id: "meetup",
        name: "交流分享",
        category: "学习交流",
        aliases: &[
            "交流分享",
            "技术分享",
            "分享会",
            "交流会",
            "沙龙",
            "meetup",
            "seminar",
        ],
        title: "主题交流会",
        capacity: 20,
        description: "主题、分享者、适合人群和交流形式由组织者确认。欢迎提前准备问题。",
        agenda: &["介绍主题与参与者", "主题分享与讨论", "问答及要点整理"],
        preparation: &["确认主题、分享者与议程", "准备演示设备及讨论问题"],
    },
    ActivityType {
        id: "volunteering",
        name: "公益志愿",
        category: "社群共建",
        aliases: &[
            "公益志愿",
            "公益",
            "志愿",
            "志愿服务",
            "volunteering",
            "volunteer",
        ],
        title: "公益志愿活动",
        capacity: 15,
        description: "服务内容、地点、分工和对接方由组织者确认。报名不代表取得官方志愿服务认证。",
        agenda: &["签到与服务说明", "按确认分工开展服务", "清点物资与服务总结"],
        preparation: &["确认对接方、工作内容及所需物资", "说明集合、分工及结束安排"],
    },
    ActivityType {
        id: "family",
        name: "亲子活动",
        category: "聚会生活",
        aliases: &["亲子活动", "亲子", "家庭活动", "family", "family activity"],
        title: "亲子交流活动",
        capacity: 12,
        description: "适合年龄、内容和场地由组织者确认。同行人数包括所有参与者，未成年人由监护人陪同。",
        agenda: &["家庭签到与活动说明", "按约定开展互动", "整理场地与结束"],
        preparation: &[
            "说明适合年龄、监护与材料安排",
            "确认按实际人数计入名额，不拆分同行报名",
        ],
    },
    ActivityType {
        id: "lecture", name: "校园讲座", category: "校园活动",
        aliases: &["校园讲座", "讲座", "学术讲座", "lecture", "campus lecture"],
        title: "校园主题讲座", capacity: 30,
        description: "围绕明确主题开展讲授与问答。主题、面向人群与主讲安排由组织者核对后公开。",
        agenda: &["签到与主题介绍", "主题讲授", "提问与交流", "材料与后续安排说明"],
        preparation: &["确认主题、主讲人、场地与投影音响", "说明面向人群、入场安排与报名截止", "准备问答时间与可分享材料"],
    },
    ActivityType {
        id: "competition", name: "校园竞赛", category: "校园活动",
        aliases: &["校园竞赛", "竞赛", "比赛", "挑战赛", "黑客松", "competition", "contest", "hackathon"],
        title: "校园创意竞赛", capacity: 24,
        description: "先明确个人赛或团队赛、赛制与提交要求。整队报名与递补，名额按实际参与人数计算。",
        agenda: &["规则与题目说明", "按约定分组开展竞赛", "提交与展示", "按已公布规则安排评审及结果通知"],
        preparation: &["确认每队人数、总参与名额与报名截止", "准备规则、提交渠道和材料清单", "说明评审安排；奖项与证书仅填写已确认内容"],
    },
    ActivityType {
        id: "music_festival", name: "校园音乐节", category: "校园活动",
        aliases: &["校园音乐节", "音乐节", "校园演出", "音乐会", "music festival", "concert"],
        title: "校园音乐节 · 小型场次", capacity: 30,
        description: "按独立场次管理观众报名或演出招募，明确参与角色、入场安排与节目。每场最多 30 个参与名额。",
        agenda: &["入场与场次说明", "按已确认节目开展演出", "互动与休息", "有序离场与设备整理"],
        preparation: &["明确本场是观众报名还是演出招募", "确认场地、音响、演出授权与秩序安排", "室外场次准备天气变化方案，仅公布已确认节目"],
    },
    ActivityType {
        id: "club", name: "社团招新", category: "校园活动",
        aliases: &["社团招新", "招新", "社团体验", "club recruitment", "club"],
        title: "社团开放体验日", capacity: 20,
        description: "让同学了解社团并参加体验活动。说明体验内容、准备事项与后续加入方式。",
        agenda: &["社团介绍", "体验与小组交流", "答疑", "说明后续加入方式"],
        preparation: &["确认面向人群、场地与体验材料", "安排接待与交流流程", "区分本场活动报名与社团成员录取"],
    },
    ActivityType {
        id: "custom",
        name: "自定义活动",
        category: "社群共建",
        aliases: &["自定义活动", "自定义", "其他活动", "其他", "custom"],
        title: "社群活动",
        capacity: 6,
        description: "请说明活动内容、参加须知和所需准备。",
        agenda: &["签到与说明", "按已确认的活动安排开展", "交流与结束"],
        preparation: &["确认活动内容、场地及所需物品", "说明参加须知与费用安排"],
    },
];

pub fn get(id: &str) -> Option<&'static ActivityType> {
    ALL.iter().find(|t| t.id == id)
}
pub fn valid(id: &str) -> bool {
    get(id).is_some()
}
pub fn preference_valid(id: &str) -> bool {
    id == "any" || valid(id)
}
pub fn ids() -> Vec<&'static str> {
    ALL.iter().map(|t| t.id).collect()
}
pub fn preference_ids() -> Vec<&'static str> {
    let mut values = ids();
    values.push("any");
    values
}
pub fn name(id: &str) -> &'static str {
    if id == "any" {
        "不限类型"
    } else {
        get(id).map_or("未知活动类型", |t| t.name)
    }
}
/// Only explicit labels/aliases are accepted. An unknown type never becomes `custom` implicitly.
pub fn parse(value: &str) -> Result<String> {
    let v = value.trim().to_ascii_lowercase();
    if ["any", "不限", "不限类型", "全部类型"].contains(&v.as_str()) {
        return Ok("any".into());
    }
    ALL.iter()
        .find(|t| t.id == v || t.name == v || t.aliases.contains(&v.as_str()))
        .map(|t| t.id.to_string())
        .ok_or_else(|| {
            "请选择活动库中的类型；其他内容可选“自定义活动”，目标与筛选也可选“不限类型”".into()
        })
}
pub fn search(query: &str) -> Vec<&'static ActivityType> {
    let q = query.trim().to_ascii_lowercase();
    ALL.iter()
        .filter(|t| {
            q.is_empty()
                || matches!(q.as_str(), "校园" | "校园活动" | "campus") && crate::campus::supported(t.id)
                || t.id.contains(&q)
                || t.name.contains(&q)
                || t.category.contains(&q)
                || t.aliases.iter().any(|a| a.contains(&q))
        })
        .collect()
}
pub fn model_catalog() -> Value {
    Value::Array(
        ALL.iter()
            .map(|t| json!({"id":t.id,"name":t.name,"category":t.category,"aliases":t.aliases}))
            .collect(),
    )
}
/// A model's type correction must cite words supporting the proposed type, not another type.
pub fn evidence_supports(id: &str, evidence: &str) -> bool {
    let text = evidence.to_ascii_lowercase();
    let aliases: &[&str] = if id == "any" {
        &["不限", "any", "全部类型"]
    } else if let Some(t) = get(id) {
        t.aliases
    } else {
        return false;
    };
    aliases.iter().any(|a| {
        text.match_indices(a).any(|(start, _)| {
            !a.is_ascii() || {
                let end = start + a.len();
                let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
                (start == 0 || !word(text.as_bytes()[start - 1]))
                    && (end == text.len() || !word(text.as_bytes()[end]))
            }
        })
    })
}
