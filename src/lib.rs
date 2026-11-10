//! MyFax — native fax inbox for printers that forward received faxes
//! (Epson "save to network folder" and similar) as PDF / TIFF files.
#![forbid(unsafe_code)]

pub mod app;
pub mod archive;
pub mod config;
pub mod i18n;
pub mod model;
pub mod pdf;
pub mod store;
pub mod system;
pub mod ui;
