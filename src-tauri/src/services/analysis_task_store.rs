use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::ai::{AnalysisBatchStatus, AnalysisFailure, AnalysisTaskSnapshot};

#[derive(Debug, Default)]
pub struct AnalysisTaskStore {
    tasks: Mutex<HashMap<String, AnalysisTaskSnapshot>>,
}

impl AnalysisTaskStore {
    pub fn create(&self, files: Vec<String>, created_at: SystemTime) -> Result<String, String> {
        if files.is_empty() {
            return Err("分析批次至少需要一个文件".into());
        }
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| "分析任务状态不可用".to_string())?;
        if tasks.values().any(|task| {
            matches!(
                task.status,
                AnalysisBatchStatus::Queued
                    | AnalysisBatchStatus::Running
                    | AnalysisBatchStatus::Cancelling
            )
        }) {
            return Err("当前已有分析批次正在运行".into());
        }
        let batch_id = format!(
            "analysis-{}-{}",
            created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            tasks.len()
        );
        tasks.insert(
            batch_id.clone(),
            AnalysisTaskSnapshot {
                batch_id: batch_id.clone(),
                status: AnalysisBatchStatus::Queued,
                total_files: files.len(),
                completed_files: 0,
                current_path: None,
                result_ids: Vec::new(),
                failures: Vec::new(),
                error: None,
            },
        );
        Ok(batch_id)
    }

    pub fn mark_running(&self, batch_id: &str) -> Result<(), String> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| "分析任务状态不可用".to_string())?;
        let task = tasks
            .get_mut(batch_id)
            .ok_or_else(|| "分析批次不存在".to_string())?;
        if task.status != AnalysisBatchStatus::Queued {
            return Err("只有排队中的分析批次可以开始运行".into());
        }
        task.status = AnalysisBatchStatus::Running;
        Ok(())
    }

    pub fn update_progress(
        &self,
        batch_id: &str,
        completed: usize,
        current_path: Option<String>,
    ) -> Result<(), String> {
        self.mutate(batch_id, |task| {
            task.completed_files = completed.min(task.total_files);
            task.current_path = current_path;
        })
    }

    pub fn complete(&self, batch_id: &str, result_ids: Vec<String>) -> Result<(), String> {
        self.complete_with_failures(batch_id, result_ids, Vec::new())
    }

    pub fn complete_with_failures(
        &self,
        batch_id: &str,
        result_ids: Vec<String>,
        failures: Vec<AnalysisFailure>,
    ) -> Result<(), String> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| "分析任务状态不可用".to_string())?;
        let task = tasks
            .get_mut(batch_id)
            .ok_or_else(|| "分析批次不存在".to_string())?;
        if task.status != AnalysisBatchStatus::Running {
            return Err("分析批次已取消或不在运行中".into());
        }
        task.status = AnalysisBatchStatus::Completed;
        task.completed_files = task.total_files;
        task.current_path = None;
        task.result_ids = result_ids;
        task.failures = failures;
        Ok(())
    }

    pub fn fail(&self, batch_id: &str, error: String) -> Result<(), String> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| "分析任务状态不可用".to_string())?;
        let task = tasks
            .get_mut(batch_id)
            .ok_or_else(|| "分析批次不存在".to_string())?;
        if !matches!(
            task.status,
            AnalysisBatchStatus::Queued | AnalysisBatchStatus::Running
        ) {
            return Err("分析批次已取消或不在运行中".into());
        }
        task.status = AnalysisBatchStatus::Failed;
        task.current_path = None;
        task.error = Some(error);
        Ok(())
    }

    pub fn cancel(&self, batch_id: &str) -> Result<(), String> {
        self.mutate(batch_id, |task| match task.status {
            AnalysisBatchStatus::Queued => {
                task.status = AnalysisBatchStatus::Cancelled;
                task.current_path = None;
            }
            AnalysisBatchStatus::Running => {
                task.status = AnalysisBatchStatus::Cancelling;
            }
            AnalysisBatchStatus::Cancelling
            | AnalysisBatchStatus::Completed
            | AnalysisBatchStatus::Failed
            | AnalysisBatchStatus::Cancelled => {}
        })
    }

    pub fn finish_cancelled(&self, batch_id: &str) -> Result<(), String> {
        self.mutate(batch_id, |task| {
            if matches!(
                task.status,
                AnalysisBatchStatus::Queued
                    | AnalysisBatchStatus::Running
                    | AnalysisBatchStatus::Cancelling
            ) {
                task.status = AnalysisBatchStatus::Cancelled;
                task.current_path = None;
            }
        })
    }

    pub fn is_cancelled(&self, batch_id: &str) -> bool {
        self.get(batch_id).is_some_and(|task| {
            matches!(
                task.status,
                AnalysisBatchStatus::Cancelling | AnalysisBatchStatus::Cancelled
            )
        })
    }

    pub fn get(&self, batch_id: &str) -> Option<AnalysisTaskSnapshot> {
        self.tasks.lock().ok()?.get(batch_id).cloned()
    }

    fn mutate(
        &self,
        batch_id: &str,
        change: impl FnOnce(&mut AnalysisTaskSnapshot),
    ) -> Result<(), String> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| "分析任务状态不可用".to_string())?;
        let task = tasks
            .get_mut(batch_id)
            .ok_or_else(|| "分析批次不存在".to_string())?;
        change(task);
        Ok(())
    }
}
