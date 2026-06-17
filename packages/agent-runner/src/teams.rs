// Institutional Agent Teams — universal cross-sector collaboration fabric.
//
// A team is a named, governed collection of agents that represents an institutional
// entity: University, Government, Enterprise, Community, Federation, NGO, Research body,
// Healthcare org, Infrastructure utility, or Financial institution.
//
// Universal properties:
//   - Language-neutral: languages[] holds ISO 639 / BCP 47 codes (Unicode-native names allowed)
//   - Geography-neutral: regions[] holds UN M.49 or ISO 3166 codes
//   - Sector-neutral: InstitutionKind covers all major sectors
//   - Federation-aware: teams federate via did:autonomyx + signed AccessGrants
//
// Teams carry a DID at activation, align to goals, and emit fabric events on every
// state change — no ungoverned subgraph is ever created.
//
// openautonomyx.com

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ── Institution kind ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionKind {
    University,      // academic research, teaching, knowledge creation
    Government,      // policy, public service, regulation
    Enterprise,      // commercial, product, market-driven
    Community,       // open-source, cooperative, digital commons
    Federation,      // alliance of institutions (meta-institution)
    Ngo,             // mission-driven, civil society, non-profit
    Research,        // pure research lab / think tank
    Healthcare,      // clinical, biomedical, public health
    Infrastructure,  // utilities, transport, telecoms, energy
    Finance,         // banking, insurance, investment, fintech
}

impl InstitutionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstitutionKind::University     => "university",
            InstitutionKind::Government     => "government",
            InstitutionKind::Enterprise     => "enterprise",
            InstitutionKind::Community      => "community",
            InstitutionKind::Federation     => "federation",
            InstitutionKind::Ngo            => "ngo",
            InstitutionKind::Research       => "research",
            InstitutionKind::Healthcare     => "healthcare",
            InstitutionKind::Infrastructure => "infrastructure",
            InstitutionKind::Finance        => "finance",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "university"     => InstitutionKind::University,
            "government"     => InstitutionKind::Government,
            "enterprise"     => InstitutionKind::Enterprise,
            "community"      => InstitutionKind::Community,
            "federation"     => InstitutionKind::Federation,
            "ngo"            => InstitutionKind::Ngo,
            "research"       => InstitutionKind::Research,
            "healthcare"     => InstitutionKind::Healthcare,
            "infrastructure" => InstitutionKind::Infrastructure,
            "finance"        => InstitutionKind::Finance,
            _                => InstitutionKind::Enterprise,
        }
    }
}

// ── Team status ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TeamStatus {
    Forming,    // being assembled — agents joining, goals aligning
    Active,     // operational — DID issued, governance live
    Suspended,  // temporarily halted — governance review in progress
    Dissolved,  // permanently ended — records retained for audit
}

// ── Institutional Team ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstitutionTeam {
    pub id:               String,
    /// Human-readable name — any Unicode (institutions name themselves in their language)
    pub name:             String,
    /// The institution this team belongs to
    pub institution_name: String,
    pub kind:             InstitutionKind,
    /// Operating charter — mission statement, scope, and constraints
    pub charter:          String,
    /// did:autonomyx:<pubkey> — assigned at activation; signs all team accountability records
    pub did:              Option<String>,
    /// Agent IDs bound to this team
    pub agents:           Vec<String>,
    /// Goal IDs aligned to this team's mission
    pub goals:            Vec<String>,
    /// ISO 639 / BCP 47 language codes — unicode-native team operations
    pub languages:        Vec<String>,
    /// UN M.49 / ISO 3166 region codes — geographic scope
    pub regions:          Vec<String>,
    pub status:           TeamStatus,
    /// Field of work — the domain this team operates in (e.g. "healthcare", "financial services",
    /// "software engineering", "public policy"). Enables user-centric, domain-bounded routing.
    pub field_of_work:    Option<String>,
    /// Primary objective orientation — "customer_satisfaction", "operational_excellence",
    /// "research", "compliance", "innovation", "service_delivery"
    pub objective:        Option<String>,
    /// Extensible metadata — domain-specific config, federation links, etc.
    pub meta:             serde_json::Value,
    pub created_at:       DateTime<Utc>,
    pub updated_at:       DateTime<Utc>,
}

// ── Team Registry ─────────────────────────────────────────────────────────────

pub struct TeamRegistry {
    pub teams: RwLock<HashMap<String, InstitutionTeam>>,
}

impl TeamRegistry {
    pub fn new() -> Self {
        Self { teams: RwLock::new(HashMap::new()) }
    }

    pub fn create(
        &self,
        name:          &str,
        institution:   &str,
        kind:          InstitutionKind,
        charter:       &str,
        field_of_work: Option<&str>,
        objective:     Option<&str>,
    ) -> InstitutionTeam {
        let id = format!("team_{}", Uuid::new_v4().simple());
        let team = InstitutionTeam {
            id: id.clone(),
            name: name.into(),
            institution_name: institution.into(),
            kind,
            charter: charter.into(),
            did: None,
            agents: vec![],
            goals: vec![],
            languages: vec!["en".into()],
            regions: vec![],
            status: TeamStatus::Forming,
            field_of_work: field_of_work.map(Into::into),
            objective:     objective.map(Into::into),
            meta: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.teams.write().unwrap().insert(id, team.clone());
        team
    }

    pub fn get(&self, id: &str) -> Option<InstitutionTeam> {
        self.teams.read().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<InstitutionTeam> {
        self.teams.read().unwrap().values().cloned().collect()
    }

    pub fn add_agent(&self, team_id: &str, agent_id: &str) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            if !t.agents.contains(&agent_id.to_string()) {
                t.agents.push(agent_id.into());
                t.updated_at = Utc::now();
            }
            return true;
        }
        false
    }

    pub fn remove_agent(&self, team_id: &str, agent_id: &str) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            let before = t.agents.len();
            t.agents.retain(|a| a != agent_id);
            if t.agents.len() != before { t.updated_at = Utc::now(); }
            return true;
        }
        false
    }

    pub fn activate(&self, team_id: &str, did: Option<&str>) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            t.status = TeamStatus::Active;
            if let Some(d) = did { t.did = Some(d.into()); }
            t.updated_at = Utc::now();
            return true;
        }
        false
    }

    pub fn add_goal(&self, team_id: &str, goal_id: &str) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            if !t.goals.contains(&goal_id.to_string()) {
                t.goals.push(goal_id.into());
                t.updated_at = Utc::now();
            }
            return true;
        }
        false
    }

    pub fn set_languages(&self, team_id: &str, langs: Vec<String>) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            t.languages = langs;
            t.updated_at = Utc::now();
            return true;
        }
        false
    }

    pub fn add_region(&self, team_id: &str, region: &str) -> bool {
        let mut teams = self.teams.write().unwrap();
        if let Some(t) = teams.get_mut(team_id) {
            if !t.regions.contains(&region.to_string()) {
                t.regions.push(region.into());
                t.updated_at = Utc::now();
            }
            return true;
        }
        false
    }

    /// Record a federation link between this team and a partner team.
    /// Both teams are updated so the link is visible from either side.
    pub fn federate(&self, team_id: &str, partner_id: &str) -> bool {
        let mut teams = self.teams.write().unwrap();
        let mut found = 0usize;
        for tid in [team_id, partner_id] {
            if let Some(t) = teams.get_mut(tid) {
                let other = if tid == team_id { partner_id } else { team_id };
                let mut partners: Vec<String> = t.meta.get("federated_with")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                if !partners.contains(&other.to_string()) {
                    partners.push(other.into());
                    t.meta["federated_with"] = serde_json::to_value(partners).unwrap_or_default();
                    t.updated_at = Utc::now();
                }
                found += 1;
            }
        }
        found > 0
    }

    pub fn summary(&self) -> serde_json::Value {
        let teams = self.teams.read().unwrap();
        let mut by_kind: HashMap<String, usize> = HashMap::new();
        let mut active = 0usize;
        let mut total_agents = 0usize;
        let mut langs: HashSet<String> = HashSet::new();
        let mut regions: HashSet<String> = HashSet::new();
        for t in teams.values() {
            *by_kind.entry(t.kind.as_str().into()).or_insert(0) += 1;
            if t.status == TeamStatus::Active { active += 1; }
            total_agents += t.agents.len();
            for l in &t.languages { langs.insert(l.clone()); }
            for r in &t.regions   { regions.insert(r.clone()); }
        }
        serde_json::json!({
            "total":        teams.len(),
            "active":       active,
            "by_kind":      by_kind,
            "total_agents": total_agents,
            "languages":    langs.len(),
            "regions":      regions.len(),
            "description":  "institutional agent teams — universal cross-sector collaboration fabric",
        })
    }
}
