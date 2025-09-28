//! Agent plan generation and reporting for ACP compliance
//!
//! ACP requires agent plan reporting for transparency and progress tracking:
//! 1. Generate actionable plan entries based on user request
//! 2. Report initial plan via session/update notification
//! 3. Update plan entry status as work progresses
//! 4. Connect plan entries to actual tool executions
//! 5. Provide clear visibility into agent's approach
//!
//! Plans should be realistic, specific, and trackable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Plan entry status lifecycle according to ACP specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanEntryStatus {
    /// Entry is pending execution
    #[serde(rename = "pending")]
    Pending,
    /// Entry is currently being executed
    #[serde(rename = "in_progress")]
    InProgress,
    /// Entry has been completed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Entry execution failed
    #[serde(rename = "failed")]
    Failed,
    /// Entry was cancelled before completion
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Priority levels for plan entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// High priority - critical for task completion
    #[serde(rename = "high")]
    High,
    /// Medium priority - important but not critical
    #[serde(rename = "medium")]
    Medium,
    /// Low priority - nice to have or cleanup tasks
    #[serde(rename = "low")]
    Low,
}

/// Individual plan entry representing a specific action or step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    /// Unique identifier for this plan entry
    pub id: String,
    /// Human-readable description of what this entry will accomplish
    pub content: String,
    /// Priority level for execution order and importance
    pub priority: Priority,
    /// Current execution status
    pub status: PlanEntryStatus,
    /// Optional additional context or notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Timestamp when this entry was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<std::time::SystemTime>,
    /// Timestamp when this entry was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<std::time::SystemTime>,
}

impl PlanEntry {
    /// Create a new plan entry with pending status
    pub fn new(content: String, priority: Priority) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            id: Ulid::new().to_string(),
            content,
            priority,
            status: PlanEntryStatus::Pending,
            notes: None,
            created_at: Some(now),
            updated_at: Some(now),
        }
    }

    /// Update the status of this plan entry
    pub fn update_status(&mut self, new_status: PlanEntryStatus) {
        if self.status != new_status {
            self.status = new_status;
            self.updated_at = Some(std::time::SystemTime::now());
        }
    }

    /// Add or update notes for this plan entry
    pub fn set_notes(&mut self, notes: String) {
        self.notes = Some(notes);
        self.updated_at = Some(std::time::SystemTime::now());
    }

    /// Check if this plan entry is complete (completed, failed, or cancelled)
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            PlanEntryStatus::Completed | PlanEntryStatus::Failed | PlanEntryStatus::Cancelled
        )
    }

    /// Check if this plan entry is currently being executed
    pub fn is_in_progress(&self) -> bool {
        matches!(self.status, PlanEntryStatus::InProgress)
    }
}

/// Container for all plan entries representing the complete execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    /// Unique identifier for this plan
    pub id: String,
    /// List of plan entries in execution order
    pub entries: Vec<PlanEntry>,
    /// Timestamp when this plan was created
    pub created_at: std::time::SystemTime,
    /// Optional metadata about the plan
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl AgentPlan {
    /// Create a new empty agent plan
    pub fn new() -> Self {
        Self {
            id: Ulid::new().to_string(),
            entries: Vec::new(),
            created_at: std::time::SystemTime::now(),
            metadata: None,
        }
    }

    /// Create a plan from a list of plan entries
    pub fn from_entries(entries: Vec<PlanEntry>) -> Self {
        Self {
            id: Ulid::new().to_string(),
            entries,
            created_at: std::time::SystemTime::now(),
            metadata: None,
        }
    }

    /// Add a plan entry to this plan
    pub fn add_entry(&mut self, entry: PlanEntry) {
        self.entries.push(entry);
    }

    /// Get a plan entry by ID
    pub fn get_entry(&self, id: &str) -> Option<&PlanEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Get a mutable reference to a plan entry by ID
    pub fn get_entry_mut(&mut self, id: &str) -> Option<&mut PlanEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// Update the status of a specific plan entry
    pub fn update_entry_status(&mut self, entry_id: &str, new_status: PlanEntryStatus) -> bool {
        if let Some(entry) = self.get_entry_mut(entry_id) {
            entry.update_status(new_status);
            true
        } else {
            false
        }
    }

    /// Get the next pending plan entry (highest priority first)
    pub fn next_pending_entry(&self) -> Option<&PlanEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.status == PlanEntryStatus::Pending)
            .min_by_key(|entry| &entry.priority)
    }

    /// Get count of entries by status
    pub fn count_by_status(&self, status: PlanEntryStatus) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == status)
            .count()
    }

    /// Check if all plan entries are complete
    pub fn is_complete(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|entry| entry.is_complete())
    }

    /// Get plan completion percentage (0.0 to 1.0)
    pub fn completion_percentage(&self) -> f64 {
        if self.entries.is_empty() {
            return 1.0;
        }

        let completed_count = self
            .entries
            .iter()
            .filter(|entry| entry.is_complete())
            .count();
        completed_count as f64 / self.entries.len() as f64
    }

    /// Convert plan to ACP-compliant format for session/update notifications
    pub fn to_acp_format(&self) -> serde_json::Value {
        serde_json::json!({
            "sessionUpdate": "plan",
            "entries": self.entries.iter().map(|entry| {
                serde_json::json!({
                    "content": entry.content,
                    "priority": entry.priority,
                    "status": entry.status
                })
            }).collect::<Vec<_>>()
        })
    }
}

impl Default for AgentPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan generator for analyzing user prompts and creating execution plans
pub struct PlanGenerator {
    /// Configuration for plan generation behavior
    config: PlanGeneratorConfig,
}

/// Configuration for plan generation
#[derive(Debug, Clone)]
pub struct PlanGeneratorConfig {
    /// Maximum number of plan entries to generate
    pub max_entries: usize,
    /// Default priority for plan entries when not specified
    pub default_priority: Priority,
    /// Whether to include cleanup tasks in plans
    pub include_cleanup_tasks: bool,
}

impl Default for PlanGeneratorConfig {
    fn default() -> Self {
        Self {
            max_entries: 10,
            default_priority: Priority::Medium,
            include_cleanup_tasks: false,
        }
    }
}

impl PlanGenerator {
    /// Create a new plan generator with default configuration
    pub fn new() -> Self {
        Self {
            config: PlanGeneratorConfig::default(),
        }
    }

    /// Create a new plan generator with custom configuration
    pub fn with_config(config: PlanGeneratorConfig) -> Self {
        Self { config }
    }

    /// Generate a plan based on user prompt analysis
    pub fn generate_plan(&self, prompt: &str) -> crate::Result<AgentPlan> {
        let mut plan = AgentPlan::new();

        // Basic heuristics for plan generation based on prompt content
        let steps = self.analyze_prompt_for_steps(prompt);

        for (content, priority) in steps {
            if plan.entries.len() >= self.config.max_entries {
                break;
            }
            plan.add_entry(PlanEntry::new(content, priority));
        }

        // Add metadata about the generation
        plan.metadata = Some(serde_json::json!({
            "generator_version": "1.0.0",
            "prompt_length": prompt.len(),
            "generated_entries": plan.entries.len(),
            "generation_strategy": "heuristic"
        }));

        Ok(plan)
    }

    /// Analyze prompt to identify actionable steps
    fn analyze_prompt_for_steps(&self, prompt: &str) -> Vec<(String, Priority)> {
        let mut steps = Vec::new();
        let prompt_lower = prompt.to_lowercase();

        // High priority steps - critical operations
        if prompt_lower.contains("fix")
            || prompt_lower.contains("error")
            || prompt_lower.contains("bug")
        {
            steps.push((
                "Identify and analyze the reported issue".to_string(),
                Priority::High,
            ));
            steps.push((
                "Implement fix for the identified issue".to_string(),
                Priority::High,
            ));
        }

        if prompt_lower.contains("implement")
            || prompt_lower.contains("create")
            || prompt_lower.contains("add")
        {
            steps.push((
                "Analyze requirements and design approach".to_string(),
                Priority::High,
            ));
            steps.push((
                "Implement the requested functionality".to_string(),
                Priority::High,
            ));
        }

        // Medium priority steps - important operations
        if prompt_lower.contains("test") {
            steps.push((
                "Run existing tests to verify current state".to_string(),
                Priority::Medium,
            ));
            steps.push((
                "Create additional tests if needed".to_string(),
                Priority::Medium,
            ));
        }

        if prompt_lower.contains("refactor") || prompt_lower.contains("clean") {
            steps.push((
                "Analyze code structure for improvement opportunities".to_string(),
                Priority::Medium,
            ));
            steps.push((
                "Refactor code while maintaining functionality".to_string(),
                Priority::Medium,
            ));
        }

        // Low priority steps - helpful but not critical
        if prompt_lower.contains("document") || prompt_lower.contains("explain") {
            steps.push(("Review and update documentation".to_string(), Priority::Low));
        }

        // Default generic steps if no specific patterns matched
        if steps.is_empty() {
            steps.push((
                "Analyze the request and identify required actions".to_string(),
                Priority::Medium,
            ));
            steps.push(("Execute the requested task".to_string(), Priority::Medium));
        }

        // Add validation step for most operations
        if steps.len() > 1 {
            steps.push((
                "Validate results and ensure quality".to_string(),
                Priority::Medium,
            ));
        }

        steps
    }
}

impl Default for PlanGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan manager for tracking plan state across sessions
pub struct PlanManager {
    /// Active plans by session ID
    active_plans: HashMap<String, AgentPlan>,
}

impl PlanManager {
    /// Create a new plan manager
    pub fn new() -> Self {
        Self {
            active_plans: HashMap::new(),
        }
    }

    /// Store a plan for a session
    pub fn set_plan(&mut self, session_id: String, plan: AgentPlan) {
        self.active_plans.insert(session_id, plan);
    }

    /// Get the current plan for a session
    pub fn get_plan(&self, session_id: &str) -> Option<&AgentPlan> {
        self.active_plans.get(session_id)
    }

    /// Get a mutable reference to the current plan for a session
    pub fn get_plan_mut(&mut self, session_id: &str) -> Option<&mut AgentPlan> {
        self.active_plans.get_mut(session_id)
    }

    /// Update plan entry status for a session
    pub fn update_plan_entry_status(
        &mut self,
        session_id: &str,
        entry_id: &str,
        new_status: PlanEntryStatus,
    ) -> bool {
        if let Some(plan) = self.get_plan_mut(session_id) {
            plan.update_entry_status(entry_id, new_status)
        } else {
            false
        }
    }

    /// Remove plan for a session (cleanup)
    pub fn remove_plan(&mut self, session_id: &str) -> Option<AgentPlan> {
        self.active_plans.remove(session_id)
    }

    /// Clean up plans for expired sessions
    pub fn cleanup_expired_plans(&mut self, active_session_ids: &[String]) {
        let active_set: std::collections::HashSet<_> = active_session_ids.iter().collect();
        self.active_plans
            .retain(|session_id, _| active_set.contains(session_id));
    }
}

impl Default for PlanManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_entry_creation() {
        let entry = PlanEntry::new("Test task".to_string(), Priority::High);
        assert_eq!(entry.content, "Test task");
        assert_eq!(entry.priority, Priority::High);
        assert_eq!(entry.status, PlanEntryStatus::Pending);
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_plan_entry_status_update() {
        let mut entry = PlanEntry::new("Test task".to_string(), Priority::Medium);
        assert_eq!(entry.status, PlanEntryStatus::Pending);

        entry.update_status(PlanEntryStatus::InProgress);
        assert_eq!(entry.status, PlanEntryStatus::InProgress);
        assert!(entry.is_in_progress());

        entry.update_status(PlanEntryStatus::Completed);
        assert_eq!(entry.status, PlanEntryStatus::Completed);
        assert!(entry.is_complete());
    }

    #[test]
    fn test_agent_plan_creation() {
        let mut plan = AgentPlan::new();
        assert!(plan.entries.is_empty());
        assert!(!plan.id.is_empty());

        let entry = PlanEntry::new("Test step".to_string(), Priority::High);
        plan.add_entry(entry);
        assert_eq!(plan.entries.len(), 1);
    }

    #[test]
    fn test_plan_completion_tracking() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Medium));

        assert!(!plan.is_complete());
        assert_eq!(plan.completion_percentage(), 0.0);

        // Complete first entry
        let entry_id = plan.entries[0].id.clone();
        plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
        assert_eq!(plan.completion_percentage(), 0.5);

        // Complete second entry
        let entry_id = plan.entries[1].id.clone();
        plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
        assert!(plan.is_complete());
        assert_eq!(plan.completion_percentage(), 1.0);
    }

    #[test]
    fn test_plan_generator() {
        let generator = PlanGenerator::new();

        let plan = generator
            .generate_plan("fix the authentication bug")
            .unwrap();
        assert!(!plan.entries.is_empty());
        assert!(plan
            .entries
            .iter()
            .any(|entry| entry.content.contains("issue")));

        let plan = generator
            .generate_plan("implement user registration feature")
            .unwrap();
        assert!(!plan.entries.is_empty());
        assert!(plan
            .entries
            .iter()
            .any(|entry| entry.content.contains("requirements")));
    }

    #[test]
    fn test_plan_acp_format() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Test step".to_string(), Priority::High));

        let acp_format = plan.to_acp_format();
        assert_eq!(acp_format["sessionUpdate"], "plan");
        assert!(acp_format["entries"].is_array());
        assert_eq!(acp_format["entries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_plan_manager() {
        let mut manager = PlanManager::new();
        let plan = AgentPlan::new();
        let session_id = "test_session".to_string();

        manager.set_plan(session_id.clone(), plan);
        assert!(manager.get_plan(&session_id).is_some());

        manager.remove_plan(&session_id);
        assert!(manager.get_plan(&session_id).is_none());
    }
}
