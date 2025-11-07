use std::{collections::HashMap, sync::Arc};

use oauth2::CsrfToken;
use tokio::sync::Mutex;

use crate::session::SessionId;

#[derive(Clone, Default)]
pub struct Db {
    inner: Arc<Mutex<DbInner>>,
}

#[derive(Default)]
struct DbInner {
    sessions: HashMap<SessionId, SessionState>,
}

impl Db {
    pub async fn create_session(&self) -> SessionId {
        let session_id = SessionId(uuid::Uuid::new_v4());
        let session = SessionState::default();
        let mut guard = self.inner.lock().await;
        guard.sessions.insert(session_id.clone(), session);
        session_id
    }

    pub async fn load_or_create_session(&self, id: &SessionId) -> Option<SessionId> {
        let mut guard = self.inner.lock().await;
        if guard.sessions.contains_key(id) {
            return None;
        }
        let session_id = SessionId(uuid::Uuid::new_v4());
        let session = SessionState::default();
        guard.sessions.insert(session_id.clone(), session);
        Some(session_id)
    }

    pub async fn set_session_csrf(
        &self,
        id: &SessionId,
        csrf: CsrfToken,
    ) -> Result<(), &'static str> {
        let mut guard = self.inner.lock().await;
        let session = guard.sessions.get_mut(id).ok_or("session does not eist")?;
        session.oauth_csrf = Some(csrf);
        Ok(())
    }

    pub async fn check_session_csrf(
        &self,
        id: &SessionId,
        csrf: CsrfToken,
    ) -> Result<(), &'static str> {
        let mut guard = self.inner.lock().await;
        let session = guard.sessions.get_mut(id).ok_or("session does not eist")?;
        if !constant_time_eq::constant_time_eq(
            session
                .oauth_csrf
                .as_ref()
                .ok_or("session does not have csrf")?
                .secret()
                .as_bytes(),
            csrf.secret().as_bytes(),
        ) {
            return Err("session csrf does not match");
        }
        session.oauth_csrf.take();
        Ok(())
    }

    pub async fn auth_user_session(&self, id: &SessionId, user: super::User) -> Result<(), ()> {
        let mut guard = self.inner.lock().await;
        let session = guard.sessions.get_mut(id).ok_or(())?;
        session.user = Some(user);
        Ok(())
    }

    pub async fn get_user(&self, id: &SessionId) -> Option<super::User> {
        let guard = self.inner.lock().await;
        guard.sessions.get(id).and_then(|s| s.user.clone())
    }
}

#[derive(Clone, Default)]
struct SessionState {
    oauth_csrf: Option<CsrfToken>,
    user: Option<super::User>,
}
