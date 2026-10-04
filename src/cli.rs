use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "nkotsu",
    version,
    about = "名古屋市交通局の市バス・地下鉄情報",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true, conflicts_with = "raw")]
    pub json: bool,
    #[arg(long, global = true)]
    pub raw: bool,
    #[arg(long, global = true)]
    pub refresh: bool,
    #[arg(long, global = true)]
    pub no_cache: bool,
    #[arg(long, global = true, default_value = "10", value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: u64,
    #[arg(short = 'v', long, global = true, conflicts_with = "quiet")]
    pub verbose: bool,
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,
    #[arg(long, global = true)]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "運行情報")]
    Status(Status),
    #[command(about = "市バス")]
    Bus {
        #[command(subcommand)]
        command: Bus,
    },
    #[command(about = "地下鉄")]
    Subway {
        #[command(subcommand)]
        command: Subway,
    },
    #[command(about = "経路検索")]
    Route(Route),
}

#[derive(Debug, Args)]
pub struct Status {
    #[arg(long)]
    pub line: Option<String>,
    #[arg(long)]
    pub bus: bool,
    #[arg(long)]
    pub subway: bool,
}

#[derive(Debug, Subcommand)]
pub enum Bus {
    #[command(about = "停留所とのりば")]
    Stop { stop: String },
    #[command(about = "時刻表")]
    Timetable {
        stop: String,
        #[command(flatten)]
        filter: BusTimetable,
    },
    #[command(about = "接近情報（到着予測ではありません）")]
    Live {
        stop: String,
        #[arg(long)]
        route: Option<String>,
        #[arg(long)]
        pole: Option<String>,
        #[arg(long)]
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
    #[arg(long)]
    pub day: Option<BusDayType>,
    #[arg(long)]
    pub pole: Option<String>,
    #[arg(long)]
    pub route: Option<String>,
    #[arg(long, value_parser = service_time)]
    pub after: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: Option<u32>,
}

#[derive(Debug, Subcommand)]
pub enum Subway {
    #[command(about = "時刻表")]
    Timetable {
        station: String,
        #[command(flatten)]
        filter: SubwayFilter,
        #[arg(long, value_parser = service_time)]
        after: Option<u32>,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        limit: Option<u32>,
    },
    #[command(about = "時刻表から算出した次発の予定列車")]
    Next {
        station: String,
        #[command(flatten)]
        filter: SubwayFilter,
        #[arg(long, value_parser = service_time)]
        at: Option<u32>,
        #[arg(long, default_value = "5", value_parser = clap::value_parser!(u32).range(1..))]
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
    #[arg(long)]
    pub line: Option<String>,
    #[arg(long)]
    pub direction: Option<String>,
    #[arg(long)]
    pub day: Option<SubwayDayType>,
}

#[derive(Debug, Args)]
pub struct Route {
    pub from: String,
    pub to: String,
    #[arg(long, conflicts_with_all = ["first", "last"])]
    pub at: Option<String>,
    #[arg(long, conflicts_with_all = ["first", "last"])]
    pub arrive: bool,
    #[arg(long, conflicts_with = "last")]
    pub first: bool,
    #[arg(long)]
    pub last: bool,
    #[arg(long)]
    pub via: Option<String>,
    #[arg(long)]
    pub bus: bool,
    #[arg(long)]
    pub subway: bool,
    #[arg(long)]
    pub slow_transfer: bool,
    #[arg(long)]
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
