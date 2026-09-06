use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::models::operation::{OperationPreview, OperationPreviewItem};

const PLAN_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanState {
    Valid,
    Consumed,
    Canceled,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanToken {
    pub plan_id: String,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlan {
    pub plan_id: String,
    pub items: Vec<OperationPreviewItem>,
}

#[derive(Debug)]
struct StoredPlan {
    expires_at: SystemTime,
    items: Vec<OperationPreviewItem>,
    state: PlanState,
}

#[derive(Debug, Default)]
pub struct PlanStore {
    plans: Mutex<HashMap<String, StoredPlan>>,
}

impl PlanStore {
    pub fn has_target_directory(&self, directory: &Path) -> Result<bool, String> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| "计划状态不可用。".to_string())?;
        Ok(plans.values().any(|plan| {
            plan.state == PlanState::Valid
                && plan.items.iter().any(|item| {
                    item.target_path
                        .parent()
                        .is_some_and(|parent| parent == directory)
                })
        }))
    }

    pub fn create(&self, preview: OperationPreview) -> Result<PlanToken, String> {
        self.create_at(preview, SystemTime::now())
    }

    pub fn create_at(
        &self,
        preview: OperationPreview,
        created_at: SystemTime,
    ) -> Result<PlanToken, String> {
        if !preview.can_confirm || preview.items.is_empty() {
            return Err("只有完整通过校验的操作才能生成计划。".into());
        }
        let expires_at = created_at + PLAN_TTL;
        let plan_id = format!(
            "plan-{}-{}",
            created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            self.plans
                .lock()
                .map_err(|_| "计划状态不可用。".to_string())?
                .len()
        );
        self.plans
            .lock()
            .map_err(|_| "计划状态不可用。".to_string())?
            .insert(
                plan_id.clone(),
                StoredPlan {
                    expires_at,
                    items: preview.items,
                    state: PlanState::Valid,
                },
            );
        Ok(PlanToken {
            plan_id,
            expires_at,
        })
    }

    pub fn consume(&self, plan_id: &str, now: SystemTime) -> Result<ValidatedPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "计划状态不可用。".to_string())?;
        let plan = plans
            .get_mut(plan_id)
            .ok_or_else(|| "操作计划不存在或已失效。".to_string())?;
        if plan.state == PlanState::Valid && now >= plan.expires_at {
            plan.state = PlanState::Expired;
        }
        if plan.state != PlanState::Valid {
            return Err(format!("操作计划当前状态为 {:?}。", plan.state));
        }
        plan.state = PlanState::Consumed;
        Ok(ValidatedPlan {
            plan_id: plan_id.into(),
            items: plan.items.clone(),
        })
    }

    pub fn cancel(&self, plan_id: &str) -> Result<(), String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "计划状态不可用。".to_string())?;
        let plan = plans
            .get_mut(plan_id)
            .ok_or_else(|| "操作计划不存在或已失效。".to_string())?;
        if plan.state != PlanState::Valid {
            return Err(format!("操作计划当前状态为 {:?}。", plan.state));
        }
        plan.state = PlanState::Canceled;
        Ok(())
    }

    pub fn state(&self, plan_id: &str, now: SystemTime) -> PlanState {
        let Ok(mut plans) = self.plans.lock() else {
            return PlanState::Unknown;
        };
        let Some(plan) = plans.get_mut(plan_id) else {
            return PlanState::Unknown;
        };
        if plan.state == PlanState::Valid && now >= plan.expires_at {
            plan.state = PlanState::Expired;
        }
        plan.state
    }
}
