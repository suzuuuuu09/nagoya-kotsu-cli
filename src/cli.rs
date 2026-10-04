use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "nkotsu",
    version,
    about = "名古屋市交通局の市バス・地下鉄情報を取得するCLI",
    after_help = "Examples:\n  nkotsu status\n  nkotsu bus live 上社\n  nkotsu subway next 藤が丘\n  nkotsu route 藤が丘 名古屋 --subway\n  nkotsu route 藤が丘 名古屋 --subway --json\n\nNotes:\n  --json と --raw は同時指定できません。エラー・警告・診断はstderrへ出します。\n\nDetailed documentation:\n  nkotsu docs list\n  nkotsu docs show <name>",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help_heading = "Global Options",
        help = "機械処理向けJSONで出力",
        conflicts_with = "raw"
    )]
    pub json: bool,
    #[arg(
        long,
        global = true,
        help_heading = "Global Options",
        help = "内部APIの生レスポンスをURLごとに出力"
    )]
    pub raw: bool,
    #[arg(
        long,
        global = true,
        help_heading = "Global Options",
        help = "キャッシュを再検証して取得"
    )]
    pub refresh: bool,
    #[arg(
        long,
        global = true,
        help_heading = "Global Options",
        help = "永続キャッシュの読み書きを無効化"
    )]
    pub no_cache: bool,
    #[arg(long, global = true, help_heading = "Global Options", help = "HTTPタイムアウト（秒、1以上）", value_name = "SECONDS", default_value = "10", value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: u64,
    #[arg(
        short = 'v',
        long,
        global = true,
        help_heading = "Global Options",
        help = "HTTP取得・エラーの詳細診断を表示",
        conflicts_with = "quiet"
    )]
    pub verbose: bool,
    #[arg(
        short = 'q',
        long,
        global = true,
        help_heading = "Global Options",
        help = "補助メッセージを抑制（結果・エラー・警告は表示）"
    )]
    pub quiet: bool,
    #[arg(
        long,
        global = true,
        help_heading = "Global Options",
        help = "色を付けずに表示（通常も色なし）"
    )]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(
        about = "運行情報を表示",
        long_about = "市バス・地下鉄の運行状況や運行変更の記事を表示します。記事作成日時とAPI取得日時は別です。",
        after_help = "Examples:\n  nkotsu status\n  nkotsu status --line 東山線 --json\n\nNotes:\n  有効な路線で記事が0件の場合も成功です。取得失敗を平常運行と断定しません。\n\nMore:\n  nkotsu docs show output"
    )]
    Status(Status),
    #[command(
        about = "市バスの停留所・時刻表・接近情報",
        long_about = "市バスの情報を取得します。停留所とのりば、時刻表、バスの接近情報を名前で調べられます。",
        after_help = "Examples:\n  nkotsu bus stop 上社\n  nkotsu bus timetable 上社\n  nkotsu bus live 上社\n\nMore:\n  nkotsu docs show bus"
    )]
    Bus {
        #[command(subcommand)]
        command: Bus,
    },
    #[command(
        about = "地下鉄の時刻表・次発列車",
        long_about = "地下鉄の時刻表と、時刻表から算出した次発の予定列車を取得します。",
        after_help = "Examples:\n  nkotsu subway timetable 藤が丘\n  nkotsu subway next 藤が丘 --limit 3\n\nNotes:\n  次発はリアルタイムの列車位置ではありません。\n\nMore:\n  nkotsu docs show subway"
    )]
    Subway {
        #[command(subcommand)]
        command: Subway,
    },
    #[command(
        about = "出発地から到着地までの経路を検索",
        long_about = "出発地から到着地までの経路を検索します。交通手段・経由地・検索時刻を指定できます。",
        after_help = "Examples:\n  nkotsu route 藤が丘 名古屋 --subway\n  nkotsu route 藤が丘 名古屋 --subway --at 09:00\n  nkotsu route 藤が丘 名古屋 --subway --at 18:00 --arrive\n  nkotsu route 藤が丘 名古屋 --subway --via 栄\n  nkotsu route 藤が丘 名古屋 --subway --json\n\nNotes:\n  --at HH:MM は日本時間の今日として扱い、過去時刻でも翌日へ繰り越しません。\n  同名の交通施設は自動選択しません。交通手段指定は地点候補と乗車区間の両方に適用します。\n  --bus だけなら市バスのみ、--subway だけなら地下鉄のみ。無指定・両方指定では両方を対象にします。\n\nMore:\n  nkotsu docs show route"
    )]
    Route(Route),
    #[command(
        about = "nkotsuの詳細ドキュメントを表示",
        long_about = "バイナリに内蔵された詳細ドキュメントを表示します。通信や永続キャッシュは使用しません。",
        after_help = "Examples:\n  nkotsu docs list\n  nkotsu docs show output\n\nNotes:\n  --json で文書を機械処理向けに取得できます。--raw は使用できません。\n\nMore:\n  nkotsu docs show output"
    )]
    Docs {
        #[command(subcommand)]
        command: Docs,
    },
}

#[derive(Debug, Args)]
pub struct Status {
    #[arg(long, help = "路線を指定（例: 東山線）", value_name = "LINE")]
    pub line: Option<String>,
    #[arg(long, help = "市バスの運行情報に絞り込み（両方指定では絞り込まない）")]
    pub bus: bool,
    #[arg(long, help = "地下鉄の運行情報に絞り込み（両方指定では絞り込まない）")]
    pub subway: bool,
}

#[derive(Debug, Subcommand)]
pub enum Bus {
    #[command(
        about = "停留所とのりばを表示",
        long_about = "指定した市バス停留所のIDとのりばを表示します。",
        after_help = "Examples:\n  nkotsu bus stop 上社\n  nkotsu bus stop 藤が丘 --json\n\nNotes:\n  名前が曖昧な場合は候補を表示し、自動選択しません。\n\nMore:\n  nkotsu docs show bus"
    )]
    Stop {
        #[arg(help = "バス停名（例: 上社、藤が丘）")]
        stop: String,
    },
    #[command(
        about = "停留所の時刻表を表示",
        long_about = "指定した停留所の予定時刻を表示します。営業日は04:00から翌04:00までで、00〜03時台は24〜27時台として扱います。",
        after_help = "Examples:\n  nkotsu bus timetable 上社\n  nkotsu bus timetable 上社 --day weekday --after 14:00 --limit 10\n\nNotes:\n  自動の日種判定ができない場合は --day を指定してください。有効な条件で0件の場合も成功です。\n\nMore:\n  nkotsu docs show bus"
    )]
    Timetable {
        #[arg(help = "バス停名（例: 上社、藤が丘）")]
        stop: String,
        #[command(flatten)]
        filter: BusTimetable,
    },
    #[command(
        about = "バスの接近情報を表示",
        long_about = "指定した停留所のバス接近情報を表示します。\n\nこれは到着予測ではありません。公式サイトの現在位置情報と通過履歴を表示します。",
        after_help = "Examples:\n  nkotsu bus live 上社\n  nkotsu bus live 上社 --route 上社12\n  nkotsu bus live 上社 --pole 4番\n  nkotsu bus live 上社 --route 上社12 --json\n\nNotes:\n  到着予測時刻・遅延分数・GPS座標を推定して表示しません。\n  現在位置が取得できない場合は「現在位置情報なし」と通過履歴を表示します。\n  通常は未通過・位置関係不明の車両が対象です。--all でも系統・のりばの絞り込みは維持します。\n\nMore:\n  nkotsu docs show bus"
    )]
    Live {
        #[arg(help = "バス停名（例: 上社、藤が丘）")]
        stop: String,
        #[arg(long, help = "系統名で絞り込み（例: 上社12）", value_name = "ROUTE")]
        route: Option<String>,
        #[arg(long, help = "のりばで絞り込み（例: 4番）", value_name = "POLE")]
        pole: Option<String>,
        #[arg(long, help = "対象停留所を通過済みのバスも表示")]
        all: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BusDayType {
    Weekday,
    Saturday,
    Holiday,
}

#[derive(Debug, Args)]
pub struct BusTimetable {
    #[arg(
        long,
        help = "使用する日種（省略時は自動判定、不明なら指定が必要）",
        value_name = "DAY"
    )]
    pub day: Option<BusDayType>,
    #[arg(long, help = "のりばで絞り込み（例: 4番）", value_name = "POLE")]
    pub pole: Option<String>,
    #[arg(long, help = "系統名で絞り込み（例: 上社12）", value_name = "ROUTE")]
    pub route: Option<String>,
    #[arg(long, value_parser = service_time, help = "指定時刻以降の予定便を表示（00:00〜27:59）", value_name = "HH:MM")]
    pub after: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "表示件数（1以上）", value_name = "N")]
    pub limit: Option<u32>,
}

#[derive(Debug, Subcommand)]
pub enum Subway {
    #[command(
        about = "地下鉄駅の時刻表を表示",
        long_about = "指定した駅の予定時刻を表示します。営業日は04:00から翌04:00までで、00〜03時台は24〜27時台として扱います。",
        after_help = "Examples:\n  nkotsu subway timetable 藤が丘\n  nkotsu subway timetable 藤が丘 --day weekday --after 14:00 --limit 10\n\nNotes:\n  自動の日種判定ができない場合は --day を指定してください。有効な条件で0件の場合も成功です。\n\nMore:\n  nkotsu docs show subway"
    )]
    Timetable {
        #[arg(help = "地下鉄駅名（例: 藤が丘、名古屋）")]
        station: String,
        #[command(flatten)]
        filter: SubwayFilter,
        #[arg(long, value_parser = service_time, help = "指定時刻以降の予定便を表示（00:00〜27:59）", value_name = "HH:MM")]
        after: Option<u32>,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "表示件数（1以上）", value_name = "N")]
        limit: Option<u32>,
    },
    #[command(
        about = "時刻表から次に発車する予定列車を表示",
        long_about = "時刻表から次に発車する予定列車を表示します。\n\nリアルタイムの列車位置ではありません。",
        after_help = "Examples:\n  nkotsu subway next 藤が丘\n  nkotsu subway next 藤が丘 --limit 3\n  nkotsu subway next 栄 --line 東山線 --direction 藤が丘方面\n\nNotes:\n  営業日は04:00で区切り、翌営業日まで検索します。--day の指定は最初の営業日だけに適用します。\n  自動の日種判定ができない場合は --day を指定してください。\n\nMore:\n  nkotsu docs show subway"
    )]
    Next {
        #[arg(help = "地下鉄駅名（例: 藤が丘、名古屋）")]
        station: String,
        #[command(flatten)]
        filter: SubwayFilter,
        #[arg(long, value_parser = service_time, value_name = "HH:MM", help = "指定時刻以降の列車を検索（省略時は現在時刻、00:00〜27:59）")]
        at: Option<u32>,
        #[arg(long, default_value = "5", value_parser = clap::value_parser!(u32).range(1..), help = "表示件数（1以上）", value_name = "N")]
        limit: u32,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SubwayDayType {
    Weekday,
    Holiday,
    NewYear,
    AllNight,
}

#[derive(Debug, Args)]
pub struct SubwayFilter {
    #[arg(long, help = "路線を指定（例: 東山線）", value_name = "LINE")]
    pub line: Option<String>,
    #[arg(long, help = "方面を指定（例: 藤が丘方面）", value_name = "DIRECTION")]
    pub direction: Option<String>,
    #[arg(
        long,
        help = "使用する日種（省略時は自動判定、不明なら指定が必要）",
        value_name = "DAY"
    )]
    pub day: Option<SubwayDayType>,
}

#[derive(Debug, Args)]
pub struct Route {
    #[arg(help = "出発地（例: 藤が丘）")]
    pub from: String,
    #[arg(help = "到着地（例: 名古屋）")]
    pub to: String,
    #[arg(long, conflicts_with_all = ["first", "last"], value_name = "TIME", help = "検索時刻（HH:MM または YYYY-MM-DDTHH:MM）")]
    pub at: Option<String>,
    #[arg(long, conflicts_with_all = ["first", "last"], help = "--atで指定した時刻までに到着する経路を検索")]
    pub arrive: bool,
    #[arg(long, conflicts_with = "last", help = "始発を検索")]
    pub first: bool,
    #[arg(long, help = "終発を検索")]
    pub last: bool,
    #[arg(long, help = "経由地を指定", value_name = "STATION")]
    pub via: Option<String>,
    #[arg(long, help = "市バスのみを検索対象にする（--subway も指定すると両方）")]
    pub bus: bool,
    #[arg(long, help = "地下鉄のみを検索対象にする（--bus も指定すると両方）")]
    pub subway: bool,
    #[arg(long, help = "ゆっくり乗換を使用")]
    pub slow_transfer: bool,
    #[arg(long, help = "のりば・区間運賃など詳細を表示")]
    pub details: bool,
}

pub fn service_time(s: &str) -> Result<u32, String> {
    let (h, m) = s.split_once(':').ok_or("時刻は HH:MM で指定してください")?;
    let h = h
        .parse::<u32>()
        .map_err(|_| "時刻は HH:MM で指定してください")?;
    let m = m
        .parse::<u32>()
        .map_err(|_| "時刻は HH:MM で指定してください")?;
    if h > 27 || m > 59 {
        return Err("時刻は00:00〜27:59で指定してください".into());
    }
    Ok((if h < 4 { h + 24 } else { h }) * 60 + m)
}

#[derive(Debug, Subcommand)]
pub enum Docs {
    #[command(
        about = "利用できる内蔵ドキュメントを一覧表示",
        after_help = "Examples:\n  nkotsu docs list\n  nkotsu docs list --json\n\nMore:\n  nkotsu docs show output"
    )]
    List,
    #[command(
        about = "指定した内蔵ドキュメントを表示",
        after_help = "Examples:\n  nkotsu docs show bus\n  nkotsu docs show output --json\n\nNotes:\n  文書名は完全一致で指定します。未知の名前は候補と終了コード3を返します。\n\nMore:\n  nkotsu docs show output"
    )]
    Show {
        #[arg(help = "文書名: bus / subway / route / output / troubleshooting")]
        name: String,
    },
}
