#![feature(let_chains)]
pub mod discord;
pub mod error;
pub mod hakumi;
pub mod mellow;
pub mod patreon;

pub use error::{ Error, Result };