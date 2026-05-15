//! Daylog's shared data layer: aw-server HTTP client, query layer,
//! aggregations, KPI math, category rules, TimeRange.

pub mod aggregate;
pub mod aw_client;
pub mod categories;
pub mod datastore;
pub mod kpi;
pub mod paths;
pub mod queries;
pub mod time;
pub mod transforms;
