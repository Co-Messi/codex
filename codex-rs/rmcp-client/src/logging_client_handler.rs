use std::sync::Arc;

use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientInfo;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CreateElicitationResult;
use rmcp::model::LoggingLevel;
use rmcp::model::LoggingMessageNotificationParam;
use rmcp::model::ProgressNotificationParam;
use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::service::NotificationContext;
use rmcp::service::RequestContext;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::rmcp_client::SendElicitation;

#[derive(Debug, Clone)]
pub struct McpLoggingNotification {
    pub level: String,
    pub logger: Option<String>,
    pub data: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct LoggingClientHandler {
    client_info: ClientInfo,
    send_elicitation: Arc<SendElicitation>,
    notification_tx: mpsc::UnboundedSender<McpLoggingNotification>,
}

impl LoggingClientHandler {
    pub(crate) fn new(
        client_info: ClientInfo,
        send_elicitation: SendElicitation,
        notification_tx: mpsc::UnboundedSender<McpLoggingNotification>,
    ) -> Self {
        Self {
            client_info,
            send_elicitation: Arc::new(send_elicitation),
            notification_tx,
        }
    }
}

impl ClientHandler for LoggingClientHandler {
    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, rmcp::ErrorData> {
        (self.send_elicitation)(context.id, request)
            .await
            .map(Into::into)
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))
    }

    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!(
            "MCP server cancelled request (request_id: {}, reason: {:?})",
            params.request_id, params.reason
        );
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!(
            "MCP server progress notification (token: {:?}, progress: {}, total: {:?}, message: {:?})",
            params.progress_token, params.progress, params.total, params.message
        );
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!("MCP server resource updated (uri: {})", params.uri);
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server resource list changed");
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server tool list changed");
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server prompt list changed");
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        let LoggingMessageNotificationParam {
            level,
            logger,
            data,
        } = params;

        // Forward actionable notifications to the session mailbox so MCP servers
        // can surface peer-to-peer messages to the model (e.g. agent-peers).
        // Skip routine Info/Debug logs — those would wake idle sessions and burn
        // tokens on every verbose log line.
        let forward = matches!(
            level,
            LoggingLevel::Notice
                | LoggingLevel::Warning
                | LoggingLevel::Error
                | LoggingLevel::Critical
                | LoggingLevel::Alert
                | LoggingLevel::Emergency
        );
        if forward {
            let _ = self.notification_tx.send(McpLoggingNotification {
                level: format!("{:?}", level),
                logger: logger.clone(),
                data: data.clone(),
            });
        }

        let logger = logger.as_deref();
        match level {
            LoggingLevel::Emergency
            | LoggingLevel::Alert
            | LoggingLevel::Critical
            | LoggingLevel::Error => {
                error!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Warning => {
                warn!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Notice | LoggingLevel::Info => {
                info!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Debug => {
                debug!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
        }
    }
}
