use crate::error::Failure;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ResultData {
    pub schema_version: u8,
    pub complete: bool,
    pub data: Data,
    pub errors: Vec<Failure>,
}
impl ResultData {
    pub fn new(data: Data) -> Self {
        Self {
            schema_version: 1,
            complete: true,
            data,
            errors: Vec::new(),
        }
    }
    pub fn fail(&mut self, scope: impl Into<String>, error: crate::error::Error) {
        self.complete = false;
        self.errors.push(Failure::new(scope, error));
    }
    pub fn exit_code(&self) -> u8 {
        self.errors.iter().map(|e| e.exit_code).max().unwrap_or(0)
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Data {
    Empty,
    Documents(Vec<crate::docs::Document>),
    Document(crate::docs::Document),
    Status(Status),
    Stop(Stop),
    Timetable(Timetable),
    Live(Live),
    Route(RouteResult),
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub fetched_at: String,
    pub records: Vec<StatusRecord>,
}
#[derive(Debug, Serialize)]
pub struct StatusRecord {
    pub line: String,
    pub title: String,
    pub message: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Place {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip)]
    pub id: String,
    #[serde(skip)]
    pub latitude: f64,
    #[serde(skip)]
    pub longitude: f64,
    #[serde(skip)]
    pub search_name: String,
    #[serde(skip)]
    pub codes: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Pole {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Stop {
    pub name: String,
    pub id: String,
    pub poles: Vec<Pole>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Departure {
    pub time: String,
    pub departure: String,
    pub service_date: String,
    pub day_type: String,
    pub line: String,
    pub direction: String,
    pub platform: String,
    pub destination: Option<String>,
    pub destination_symbol: String,
    #[serde(skip)]
    pub minutes: u32,
}
#[derive(Debug, Serialize)]
pub struct Timetable {
    pub station: Place,
    pub scheduled: bool,
    pub service_date: String,
    pub day_type: String,
    pub departures: Vec<Departure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveRoute {
    #[serde(skip)]
    pub code: String,
    pub name: String,
    pub destination: String,
    pub via: String,
    pub poles: Vec<String>,
    pub vehicles: Vec<Vehicle>,
    #[serde(skip)]
    pub stops: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub from_stop: Option<String>,
    pub to_stop: Option<String>,
    pub relation: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Pass {
    pub stop: String,
    pub time: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Vehicle {
    pub vehicle: String,
    pub current_position: Option<Position>,
    pub latest_pass: Option<Pass>,
}
#[derive(Debug, Serialize)]
pub struct Live {
    pub stop: String,
    pub fetched_at: String,
    pub service_active: Option<bool>,
    pub notice: String,
    pub routes: Vec<LiveRoute>,
}

#[derive(Debug, Serialize)]
pub struct RouteResult {
    pub from: Place,
    pub to: Place,
    pub routes: Vec<Route>,
}
#[derive(Debug, Serialize)]
pub struct Route {
    pub departure: String,
    pub arrival: String,
    pub duration_minutes: Option<u32>,
    pub fare_yen: Option<u32>,
    pub riding_segments: usize,
    pub transfers: usize,
    pub segments: Vec<Segment>,
}
#[derive(Debug, Serialize)]
pub struct Segment {
    pub line: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub departure: String,
    pub arrival: String,
    pub from_platform: String,
    pub to_platform: String,
    pub fare_yen: Option<u32>,
    #[serde(skip)]
    pub kind: String,
}
