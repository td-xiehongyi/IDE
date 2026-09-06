//! Core application services live here.

pub mod analysis_service;
pub mod analysis_task_store;
pub mod content_chunker;
pub mod content_extractor;
pub mod file_identity;
pub mod operation_executor;
pub mod operation_validator;
pub mod path_policy;
pub mod plan_store;
pub mod scanner;
pub mod search;
pub mod suggestion_review;
pub mod suggestion_validator;
pub mod undo_service;
pub mod watcher;
