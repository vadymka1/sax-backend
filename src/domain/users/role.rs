use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
pub enum Role {
    SuperAdmin,
    Admin,
    Editor,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::SuperAdmin => "super_admin",
            Role::Admin => "admin",
            Role::Editor => "editor",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "super_admin" => Some(Role::SuperAdmin),
            "admin" => Some(Role::Admin),
            "editor" => Some(Role::Editor),
            _ => None,
        }
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, Role::SuperAdmin)
    }

    pub fn can_manage_content(&self) -> bool {
        matches!(self, Role::SuperAdmin | Role::Admin)
    }

    pub fn can_manage_media(&self) -> bool {
        matches!(self, Role::SuperAdmin | Role::Admin)
    }

    pub fn can_manage_roles(&self) -> bool {
        matches!(self, Role::SuperAdmin)
    }

    pub fn can_create_role(&self, target: Role) -> bool {
        match self {
            Role::SuperAdmin => target == Role::Admin,
            _ => false,
        }
    }

    pub fn can_view_audit_logs(&self) -> bool {
        matches!(self, Role::SuperAdmin | Role::Admin)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
