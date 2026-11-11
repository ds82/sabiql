use uuid::{Uuid, uuid};

use crate::domain::{ConnectionId, DatabaseType};
use crate::model::app_state::AppState;
use crate::model::shared::input_mode::InputMode;

const CLI_POSTGRES_CONNECTION_NAMESPACE: Uuid = uuid!("c011c3ef-b37f-44c5-a524-c616ce7a0a1a");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliPostgresTarget {
    uri: String,
}

impl CliPostgresTarget {
    pub fn parse_cli_argument(input: &str) -> Option<Self> {
        let is_uri = input.starts_with("postgresql://") || input.starts_with("postgres://");
        is_uri.then(|| Self {
            uri: input.to_owned(),
        })
    }

    /// `user@host/dbname` without password or query parameters.
    fn display_name(&self) -> String {
        let rest = self.uri.split_once("://").map_or(&*self.uri, |(_, r)| r);
        let rest = rest.split('?').next().unwrap_or(rest);
        match rest.rsplit_once('@') {
            Some((userinfo, host_db)) => {
                let user = userinfo.split(':').next().unwrap_or(userinfo);
                format!("{user}@{host_db}")
            }
            None => rest.to_owned(),
        }
    }

    // Derived from the password-free form so the id is stable and carries no secrets
    fn connection_id(&self) -> ConnectionId {
        let derived = Uuid::new_v5(
            &CLI_POSTGRES_CONNECTION_NAMESPACE,
            self.display_name().as_bytes(),
        );
        ConnectionId::from_string(format!("cli-postgres-{}", derived.as_simple()))
    }
}

pub fn activate_cli_postgres_connection(state: &mut AppState, target: &CliPostgresTarget) {
    state.session.activate_cli_ephemeral_connection(
        &target.connection_id(),
        &target.display_name(),
        &target.uri,
        DatabaseType::PostgreSQL,
    );
    state.modal.set_mode(InputMode::Normal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    mod parse_cli_argument {
        use super::*;

        #[rstest]
        #[case("postgresql://user:pw@localhost:5432/app")]
        #[case("postgres://localhost/app")]
        fn accepts_postgres_uris(#[case] input: &str) {
            let target = CliPostgresTarget::parse_cli_argument(input).unwrap();

            assert_eq!(target.uri, input);
        }

        #[rstest]
        #[case("")]
        #[case("app.db")]
        #[case("sqlite:///tmp/app.db")]
        #[case("mysql://localhost/app")]
        #[case("service=mydb")]
        #[case("host=localhost dbname=app")]
        fn rejects_non_postgres_uris(#[case] input: &str) {
            assert!(CliPostgresTarget::parse_cli_argument(input).is_none());
        }
    }

    mod display_name {
        use super::*;

        #[rstest]
        #[case(
            "postgresql://user:secret@db.example.com:5432/app?sslmode=require",
            "user@db.example.com:5432/app"
        )]
        #[case("postgres://user@localhost/app", "user@localhost/app")]
        #[case("postgres://localhost/app", "localhost/app")]
        #[case("postgresql://u:p%40ss@host/app", "u@host/app")]
        fn omits_password_and_query(#[case] input: &str, #[case] expected: &str) {
            let target = CliPostgresTarget::parse_cli_argument(input).unwrap();

            assert_eq!(target.display_name(), expected);
        }
    }

    mod connection_id {
        use super::*;

        #[test]
        fn ignores_password() {
            let a = CliPostgresTarget::parse_cli_argument("postgres://u:one@host/app").unwrap();
            let b = CliPostgresTarget::parse_cli_argument("postgres://u:two@host/app").unwrap();

            assert_eq!(a.connection_id(), b.connection_id());
        }
    }

    #[test]
    fn activation_sets_postgres_ephemeral_session() {
        let mut state = AppState::new("test".to_string());
        let target =
            CliPostgresTarget::parse_cli_argument("postgresql://u:pw@localhost/app").unwrap();

        activate_cli_postgres_connection(&mut state, &target);

        assert_eq!(
            state.session.active_database_type(),
            Some(DatabaseType::PostgreSQL)
        );
        assert_eq!(state.session.dsn(), Some("postgresql://u:pw@localhost/app"));
        assert!(state.session.is_ephemeral_connection());
        assert_eq!(state.input_mode(), InputMode::Normal);
    }
}
