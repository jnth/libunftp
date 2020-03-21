//! The RFC 959 Password (`PASS`) command
//
// The argument field is a Telnet string specifying the user's
// password.  This command must be immediately preceded by the
// user name command, and, for some sites, completes the user's
// identification for access control.  Since password
// information is quite sensitive, it is desirable in general
// to "mask" it or suppress typeout.  It appears that the
// server has no foolproof way to achieve this.  It is
// therefore the responsibility of the user-FTP process to hide
// the sensitive password information.

use crate::server::chancomms::InternalMsg;
use crate::server::commands::Cmd;
use crate::server::error::FTPError;
use crate::server::password;
use crate::server::reply::{Reply, ReplyCode};
use crate::server::session::SessionState;
use crate::server::CommandArgs;
use crate::storage;
use async_trait::async_trait;
use futures03::channel::mpsc::Sender;

use std::sync::Arc;

pub struct Pass {
    password: password::Password,
}

impl Pass {
    pub fn new(password: password::Password) -> Self {
        Pass { password }
    }
}

#[async_trait]
impl<S, U> Cmd<S, U> for Pass
where
    U: Send + Sync + 'static,
    S: 'static + storage::StorageBackend<U> + Sync + Send,
    S::File: crate::storage::AsAsyncReads + Send,
    S::Metadata: storage::Metadata,
{
    async fn execute(&self, args: CommandArgs<S, U>) -> Result<Reply, FTPError> {
        // let session_arc = args.session.clone();
        let mut session = args.session.lock().await;
        match &session.state {
            SessionState::WaitPass => {
                let pass: &str = std::str::from_utf8(&self.password.as_ref())?;
                let pass: String = pass.to_string();
                let user: String = session.username.clone().unwrap();
                let mut tx: Sender<InternalMsg> = args.tx.clone();

                let auther = args.authenticator.clone();
                match auther.authenticate(&user, &pass).await {
                    Ok(user) => {
                        session.user = Arc::new(Some(user));
                        tokio02::spawn(async move {
                            use futures03::prelude::*;
                            if let Err(err) = tx.send(InternalMsg::AuthSuccess).await {
                                warn!("{}", err);
                            }
                        });
                    }
                    Err(_) => {
                        tokio02::spawn(async move {
                            use futures03::prelude::*;
                            if let Err(err) = tx.send(InternalMsg::AuthFailed).await {
                                warn!("{}", err);
                            }
                        });
                    }
                };
                Ok(Reply::none())
            }
            SessionState::New => Ok(Reply::new(ReplyCode::BadCommandSequence, "Please supply a username first")),
            _ => Ok(Reply::new(ReplyCode::NotLoggedIn, "Please open a new connection to re-authenticate")),
        }
    }
}
