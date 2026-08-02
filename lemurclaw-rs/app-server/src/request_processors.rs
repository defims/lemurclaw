use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::command_exec::CommandExecManager;
use crate::command_exec::StartCommandExecParams;
use crate::config_manager::ConfigManager;
use crate::error_code::INPUT_TOO_LARGE_ERROR_CODE;
use crate::error_code::invalid_params;
use crate::models::supported_models;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::RequestContext;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::skills_watcher::SkillsWatcher;
use crate::thread_status::ThreadWatchManager;
use crate::thread_status::resolve_thread_status;
use chrono::Duration as ChronoDuration;
use chrono::SecondsFormat;
use lemurclaw_analytics::AnalyticsEventsClient;
use lemurclaw_analytics::AnalyticsJsonRpcError;
use lemurclaw_analytics::InputError;
use lemurclaw_analytics::TurnSteerRequestError;
use lemurclaw_app_server_protocol::Account;
use lemurclaw_app_server_protocol::AccountLoginCompletedNotification;
use lemurclaw_app_server_protocol::AccountTokenUsageDailyBucket;
use lemurclaw_app_server_protocol::AccountTokenUsageSummary;
use lemurclaw_app_server_protocol::AccountUpdatedNotification;
use lemurclaw_app_server_protocol::AddCreditsNudgeCreditType;
use lemurclaw_app_server_protocol::AddCreditsNudgeEmailStatus;
use lemurclaw_app_server_protocol::AdditionalContextEntry;
use lemurclaw_app_server_protocol::AdditionalContextKind;
use lemurclaw_app_server_protocol::AppListUpdatedNotification;
use lemurclaw_app_server_protocol::AppSummary;
use lemurclaw_app_server_protocol::AppTemplateSummary;
use lemurclaw_app_server_protocol::AppTemplateUnavailableReason;
use lemurclaw_app_server_protocol::AppsInstalledParams;
use lemurclaw_app_server_protocol::AppsInstalledResponse;
use lemurclaw_app_server_protocol::AppsListParams;
use lemurclaw_app_server_protocol::AppsListResponse;
use lemurclaw_app_server_protocol::AppsReadParams;
use lemurclaw_app_server_protocol::AppsReadResponse;
use lemurclaw_app_server_protocol::AskForApproval;
use lemurclaw_app_server_protocol::AuthMode;
use lemurclaw_app_server_protocol::CancelLoginAccountParams;
use lemurclaw_app_server_protocol::CancelLoginAccountResponse;
use lemurclaw_app_server_protocol::CancelLoginAccountStatus;
use lemurclaw_app_server_protocol::ClientInfo;
use lemurclaw_app_server_protocol::ClientRequest;
use lemurclaw_app_server_protocol::ClientResponsePayload;
use lemurclaw_app_server_protocol::LemurclawErrorInfo;
use lemurclaw_app_server_protocol::CollaborationModeListParams;
use lemurclaw_app_server_protocol::CollaborationModeListResponse;
use lemurclaw_app_server_protocol::CommandExecParams;
use lemurclaw_app_server_protocol::CommandExecResizeParams;
use lemurclaw_app_server_protocol::CommandExecTerminateParams;
use lemurclaw_app_server_protocol::CommandExecWriteParams;
use lemurclaw_app_server_protocol::ConfigWarningNotification;
use lemurclaw_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use lemurclaw_app_server_protocol::ConsumeAccountRateLimitResetCreditParams;
use lemurclaw_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use lemurclaw_app_server_protocol::ConversationGitInfo;
use lemurclaw_app_server_protocol::ConversationSummary;
use lemurclaw_app_server_protocol::DeprecationNoticeNotification;
use lemurclaw_app_server_protocol::DynamicToolFunctionSpec;
use lemurclaw_app_server_protocol::DynamicToolNamespaceTool;
use lemurclaw_app_server_protocol::DynamicToolSpec;
use lemurclaw_app_server_protocol::EnvironmentAddParams;
use lemurclaw_app_server_protocol::EnvironmentAddResponse;
use lemurclaw_app_server_protocol::EnvironmentInfoParams;
use lemurclaw_app_server_protocol::EnvironmentInfoResponse;
use lemurclaw_app_server_protocol::EnvironmentShellInfo;
use lemurclaw_app_server_protocol::EnvironmentStatusKind;
use lemurclaw_app_server_protocol::EnvironmentStatusParams;
use lemurclaw_app_server_protocol::EnvironmentStatusResponse;
use lemurclaw_app_server_protocol::ExperimentalFeature as ApiExperimentalFeature;
use lemurclaw_app_server_protocol::ExperimentalFeatureListParams;
use lemurclaw_app_server_protocol::ExperimentalFeatureListResponse;
use lemurclaw_app_server_protocol::ExperimentalFeatureStage as ApiExperimentalFeatureStage;
use lemurclaw_app_server_protocol::FeedbackUploadParams;
use lemurclaw_app_server_protocol::FeedbackUploadResponse;
use lemurclaw_app_server_protocol::GetAccountParams;
use lemurclaw_app_server_protocol::GetAccountRateLimitsResponse;
use lemurclaw_app_server_protocol::GetAccountResponse;
use lemurclaw_app_server_protocol::GetAccountTokenUsageResponse;
use lemurclaw_app_server_protocol::GetAuthStatusParams;
use lemurclaw_app_server_protocol::GetAuthStatusResponse;
use lemurclaw_app_server_protocol::GetConversationSummaryParams;
use lemurclaw_app_server_protocol::GetConversationSummaryResponse;
use lemurclaw_app_server_protocol::GetWorkspaceMessagesResponse;
use lemurclaw_app_server_protocol::GitDiffToRemoteParams;
use lemurclaw_app_server_protocol::GitDiffToRemoteResponse;
use lemurclaw_app_server_protocol::GitInfo as ApiGitInfo;
use lemurclaw_app_server_protocol::HookMetadata;
use lemurclaw_app_server_protocol::HooksListParams;
use lemurclaw_app_server_protocol::HooksListResponse;
use lemurclaw_app_server_protocol::InitializeParams;
use lemurclaw_app_server_protocol::InitializeResponse;
use lemurclaw_app_server_protocol::InstalledApp;
use lemurclaw_app_server_protocol::JSONRPCErrorError;
use lemurclaw_app_server_protocol::ListMcpServerStatusParams;
use lemurclaw_app_server_protocol::ListMcpServerStatusResponse;
use lemurclaw_app_server_protocol::LoginAccountParams;
use lemurclaw_app_server_protocol::LoginAccountResponse;
use lemurclaw_app_server_protocol::LoginApiKeyParams;
use lemurclaw_app_server_protocol::LoginAppBrand;
use lemurclaw_app_server_protocol::LogoutAccountResponse;
use lemurclaw_app_server_protocol::MarketplaceAddParams;
use lemurclaw_app_server_protocol::MarketplaceAddResponse;
use lemurclaw_app_server_protocol::MarketplaceInterface;
use lemurclaw_app_server_protocol::MarketplaceRemoveParams;
use lemurclaw_app_server_protocol::MarketplaceRemoveResponse;
use lemurclaw_app_server_protocol::MarketplaceUpgradeErrorInfo;
use lemurclaw_app_server_protocol::MarketplaceUpgradeParams;
use lemurclaw_app_server_protocol::MarketplaceUpgradeResponse;
use lemurclaw_app_server_protocol::McpResourceReadParams;
use lemurclaw_app_server_protocol::McpResourceReadResponse;
use lemurclaw_app_server_protocol::McpServerOauthLoginCompletedNotification;
use lemurclaw_app_server_protocol::McpServerOauthLoginParams;
use lemurclaw_app_server_protocol::McpServerOauthLoginResponse;
use lemurclaw_app_server_protocol::McpServerRefreshResponse;
use lemurclaw_app_server_protocol::McpServerStatus;
use lemurclaw_app_server_protocol::McpServerStatusDetail;
use lemurclaw_app_server_protocol::McpServerToolCallParams;
use lemurclaw_app_server_protocol::McpServerToolCallResponse;
use lemurclaw_app_server_protocol::MemoryResetResponse;
use lemurclaw_app_server_protocol::MockExperimentalMethodParams;
use lemurclaw_app_server_protocol::MockExperimentalMethodResponse;
use lemurclaw_app_server_protocol::ModelListParams;
use lemurclaw_app_server_protocol::ModelListResponse;
use lemurclaw_app_server_protocol::PermissionProfileListParams;
use lemurclaw_app_server_protocol::PermissionProfileListResponse;
use lemurclaw_app_server_protocol::PermissionProfileSummary;
use lemurclaw_app_server_protocol::PluginDetail;
use lemurclaw_app_server_protocol::PluginInstallParams;
use lemurclaw_app_server_protocol::PluginInstallResponse;
use lemurclaw_app_server_protocol::PluginInstalledParams;
use lemurclaw_app_server_protocol::PluginInstalledResponse;
use lemurclaw_app_server_protocol::PluginInterface;
use lemurclaw_app_server_protocol::PluginListMarketplaceKind;
use lemurclaw_app_server_protocol::PluginListParams;
use lemurclaw_app_server_protocol::PluginListResponse;
use lemurclaw_app_server_protocol::PluginMarketplaceEntry;
use lemurclaw_app_server_protocol::PluginReadParams;
use lemurclaw_app_server_protocol::PluginReadResponse;
use lemurclaw_app_server_protocol::PluginShareCheckoutParams;
use lemurclaw_app_server_protocol::PluginShareCheckoutResponse;
use lemurclaw_app_server_protocol::PluginShareContext;
use lemurclaw_app_server_protocol::PluginShareDeleteParams;
use lemurclaw_app_server_protocol::PluginShareDeleteResponse;
use lemurclaw_app_server_protocol::PluginShareDiscoverability;
use lemurclaw_app_server_protocol::PluginShareListItem;
use lemurclaw_app_server_protocol::PluginShareListParams;
use lemurclaw_app_server_protocol::PluginShareListResponse;
use lemurclaw_app_server_protocol::PluginSharePrincipal;
use lemurclaw_app_server_protocol::PluginSharePrincipalType;
use lemurclaw_app_server_protocol::PluginShareSaveParams;
use lemurclaw_app_server_protocol::PluginShareSaveResponse;
use lemurclaw_app_server_protocol::PluginShareTarget;
use lemurclaw_app_server_protocol::PluginShareUpdateDiscoverability;
use lemurclaw_app_server_protocol::PluginShareUpdateTargetsParams;
use lemurclaw_app_server_protocol::PluginShareUpdateTargetsResponse;
use lemurclaw_app_server_protocol::PluginSkillReadParams;
use lemurclaw_app_server_protocol::PluginSkillReadResponse;
use lemurclaw_app_server_protocol::PluginSource;
use lemurclaw_app_server_protocol::PluginSummary;
use lemurclaw_app_server_protocol::PluginUninstallParams;
use lemurclaw_app_server_protocol::PluginUninstallResponse;
use lemurclaw_app_server_protocol::RateLimitResetCredit;
use lemurclaw_app_server_protocol::RateLimitResetCreditStatus;
use lemurclaw_app_server_protocol::RateLimitResetCreditsSummary;
use lemurclaw_app_server_protocol::RateLimitResetType;
use lemurclaw_app_server_protocol::RequestId;
use lemurclaw_app_server_protocol::ReviewDelivery as ApiReviewDelivery;
use lemurclaw_app_server_protocol::ReviewStartParams;
use lemurclaw_app_server_protocol::ReviewStartResponse;
use lemurclaw_app_server_protocol::ReviewTarget as ApiReviewTarget;
use lemurclaw_app_server_protocol::SandboxMode;
use lemurclaw_app_server_protocol::SendAddCreditsNudgeEmailParams;
use lemurclaw_app_server_protocol::SendAddCreditsNudgeEmailResponse;
use lemurclaw_app_server_protocol::ServerNotification;
use lemurclaw_app_server_protocol::ServerRequestResolvedNotification;
use lemurclaw_app_server_protocol::SkillSummary;
use lemurclaw_app_server_protocol::SkillsConfigWriteParams;
use lemurclaw_app_server_protocol::SkillsConfigWriteResponse;
use lemurclaw_app_server_protocol::SkillsExtraRootsSetParams;
use lemurclaw_app_server_protocol::SkillsExtraRootsSetResponse;
use lemurclaw_app_server_protocol::SkillsListParams;
use lemurclaw_app_server_protocol::SkillsListResponse;
use lemurclaw_app_server_protocol::SortDirection;
use lemurclaw_app_server_protocol::Thread;
use lemurclaw_app_server_protocol::ThreadApproveGuardianDeniedActionParams;
use lemurclaw_app_server_protocol::ThreadApproveGuardianDeniedActionResponse;
use lemurclaw_app_server_protocol::ThreadArchiveParams;
use lemurclaw_app_server_protocol::ThreadArchiveResponse;
use lemurclaw_app_server_protocol::ThreadArchivedNotification;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminal;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsCleanParams;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsListParams;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsListResponse;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsTerminateParams;
use lemurclaw_app_server_protocol::ThreadBackgroundTerminalsTerminateResponse;
use lemurclaw_app_server_protocol::ThreadClosedNotification;
use lemurclaw_app_server_protocol::ThreadCompactStartParams;
use lemurclaw_app_server_protocol::ThreadCompactStartResponse;
use lemurclaw_app_server_protocol::ThreadDecrementElicitationParams;
use lemurclaw_app_server_protocol::ThreadDecrementElicitationResponse;
use lemurclaw_app_server_protocol::ThreadDeleteParams;
use lemurclaw_app_server_protocol::ThreadDeleteResponse;
use lemurclaw_app_server_protocol::ThreadDeletedNotification;
use lemurclaw_app_server_protocol::ThreadForkParams;
use lemurclaw_app_server_protocol::ThreadForkResponse;
use lemurclaw_app_server_protocol::ThreadGoal;
use lemurclaw_app_server_protocol::ThreadGoalClearParams;
use lemurclaw_app_server_protocol::ThreadGoalClearResponse;
use lemurclaw_app_server_protocol::ThreadGoalClearedNotification;
use lemurclaw_app_server_protocol::ThreadGoalGetParams;
use lemurclaw_app_server_protocol::ThreadGoalGetResponse;
use lemurclaw_app_server_protocol::ThreadGoalSetParams;
use lemurclaw_app_server_protocol::ThreadGoalSetResponse;
use lemurclaw_app_server_protocol::ThreadGoalStatus;
use lemurclaw_app_server_protocol::ThreadGoalUpdatedNotification;
use lemurclaw_app_server_protocol::ThreadHistoryBuilder;
#[cfg(test)]
use lemurclaw_app_server_protocol::ThreadHistoryMode;
use lemurclaw_app_server_protocol::ThreadIncrementElicitationParams;
use lemurclaw_app_server_protocol::ThreadIncrementElicitationResponse;
use lemurclaw_app_server_protocol::ThreadInjectItemsParams;
use lemurclaw_app_server_protocol::ThreadInjectItemsResponse;
use lemurclaw_app_server_protocol::ThreadItem;
use lemurclaw_app_server_protocol::ThreadItemEntry;
use lemurclaw_app_server_protocol::ThreadItemsListParams;
use lemurclaw_app_server_protocol::ThreadItemsListResponse;
use lemurclaw_app_server_protocol::ThreadListCwdFilter;
use lemurclaw_app_server_protocol::ThreadListParams;
use lemurclaw_app_server_protocol::ThreadListResponse;
use lemurclaw_app_server_protocol::ThreadLoadedListParams;
use lemurclaw_app_server_protocol::ThreadLoadedListResponse;
use lemurclaw_app_server_protocol::ThreadMemoryModeSetParams;
use lemurclaw_app_server_protocol::ThreadMemoryModeSetResponse;
use lemurclaw_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use lemurclaw_app_server_protocol::ThreadMetadataUpdateParams;
use lemurclaw_app_server_protocol::ThreadMetadataUpdateResponse;
use lemurclaw_app_server_protocol::ThreadNameUpdatedNotification;
use lemurclaw_app_server_protocol::ThreadReadParams;
use lemurclaw_app_server_protocol::ThreadReadResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendAudioParams;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendSpeechParams;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendSpeechResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendTextParams;
use lemurclaw_app_server_protocol::ThreadRealtimeAppendTextResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeListVoicesResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeStartParams;
use lemurclaw_app_server_protocol::ThreadRealtimeStartResponse;
use lemurclaw_app_server_protocol::ThreadRealtimeStartTransport;
use lemurclaw_app_server_protocol::ThreadRealtimeStopParams;
use lemurclaw_app_server_protocol::ThreadRealtimeStopResponse;
use lemurclaw_app_server_protocol::ThreadResumeInitialTurnsPageParams;
use lemurclaw_app_server_protocol::ThreadResumeParams;
use lemurclaw_app_server_protocol::ThreadResumeResponse;
use lemurclaw_app_server_protocol::ThreadRollbackParams;
use lemurclaw_app_server_protocol::ThreadSearchOccurrence;
use lemurclaw_app_server_protocol::ThreadSearchOccurrencesParams;
use lemurclaw_app_server_protocol::ThreadSearchOccurrencesResponse;
use lemurclaw_app_server_protocol::ThreadSearchParams;
use lemurclaw_app_server_protocol::ThreadSearchResponse;
use lemurclaw_app_server_protocol::ThreadSearchResult;
use lemurclaw_app_server_protocol::ThreadSearchTextRange;
use lemurclaw_app_server_protocol::ThreadSetNameParams;
use lemurclaw_app_server_protocol::ThreadSetNameResponse;
use lemurclaw_app_server_protocol::ThreadSettings;
use lemurclaw_app_server_protocol::ThreadSettingsUpdateParams;
use lemurclaw_app_server_protocol::ThreadSettingsUpdateResponse;
use lemurclaw_app_server_protocol::ThreadShellCommandParams;
use lemurclaw_app_server_protocol::ThreadShellCommandResponse;
use lemurclaw_app_server_protocol::ThreadSortKey;
use lemurclaw_app_server_protocol::ThreadSourceKind;
use lemurclaw_app_server_protocol::ThreadStartParams;
use lemurclaw_app_server_protocol::ThreadStartResponse;
use lemurclaw_app_server_protocol::ThreadStartedNotification;
use lemurclaw_app_server_protocol::ThreadStatus;
use lemurclaw_app_server_protocol::ThreadTurnsListParams;
use lemurclaw_app_server_protocol::ThreadTurnsListResponse;
use lemurclaw_app_server_protocol::ThreadUnarchiveParams;
use lemurclaw_app_server_protocol::ThreadUnarchiveResponse;
use lemurclaw_app_server_protocol::ThreadUnarchivedNotification;
use lemurclaw_app_server_protocol::ThreadUnsubscribeParams;
use lemurclaw_app_server_protocol::ThreadUnsubscribeResponse;
use lemurclaw_app_server_protocol::ThreadUnsubscribeStatus;
use lemurclaw_app_server_protocol::Turn;
use lemurclaw_app_server_protocol::TurnEnvironmentParams;
use lemurclaw_app_server_protocol::TurnError;
use lemurclaw_app_server_protocol::TurnInterruptParams;
use lemurclaw_app_server_protocol::TurnInterruptResponse;
use lemurclaw_app_server_protocol::TurnItemsView;
use lemurclaw_app_server_protocol::TurnStartParams;
use lemurclaw_app_server_protocol::TurnStartResponse;
use lemurclaw_app_server_protocol::TurnStatus;
use lemurclaw_app_server_protocol::TurnSteerParams;
use lemurclaw_app_server_protocol::TurnSteerResponse;
use lemurclaw_app_server_protocol::UserInput as V2UserInput;
use lemurclaw_app_server_protocol::WindowsSandboxReadiness;
use lemurclaw_app_server_protocol::WindowsSandboxReadinessResponse;
use lemurclaw_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use lemurclaw_app_server_protocol::WindowsSandboxSetupMode;
use lemurclaw_app_server_protocol::WindowsSandboxSetupStartParams;
use lemurclaw_app_server_protocol::WindowsSandboxSetupStartResponse;
use lemurclaw_app_server_protocol::WorkspaceMessage;
use lemurclaw_app_server_protocol::WorkspaceMessageType;
use lemurclaw_arg0::Arg0DispatchPaths;
use lemurclaw_backend_client::AddCreditsNudgeCreditType as BackendAddCreditsNudgeCreditType;
use lemurclaw_backend_client::Client as BackendClient;
use lemurclaw_backend_client::CodexWorkspaceMessage as BackendWorkspaceMessage;
use lemurclaw_backend_client::CodexWorkspaceMessageType as BackendWorkspaceMessageType;
use lemurclaw_backend_client::CodexWorkspaceMessagesResponse as BackendWorkspaceMessagesResponse;
use lemurclaw_backend_client::ConsumeRateLimitResetCreditCode as BackendConsumeRateLimitResetCreditCode;
use lemurclaw_backend_client::RateLimitResetCreditDetails as BackendRateLimitResetCreditDetails;
use lemurclaw_backend_client::RateLimitResetCreditsDetails as BackendRateLimitResetCreditsDetails;
use lemurclaw_backend_client::RequestError as BackendRequestError;
use lemurclaw_backend_client::TokenUsageProfile;
use lemurclaw_chatgpt::connectors;
use lemurclaw_chatgpt::workspace_settings;
use lemurclaw_config::CloudConfigBundleLoadError;
use lemurclaw_config::CloudConfigBundleLoadErrorCode;
use lemurclaw_config::ConfigLayerStack;
use lemurclaw_config::loader::project_trust_key;
use lemurclaw_config::types::McpServerTransportConfig;
use lemurclaw_connectors::AppInfo;
use lemurclaw_core::CodexThread;
use lemurclaw_core::CodexThreadSettingsOverrides;
use lemurclaw_core::ForkSnapshot;
use lemurclaw_core::McpManager;
use lemurclaw_core::NewThread;
#[cfg(test)]
use lemurclaw_core::SessionMeta;
use lemurclaw_core::StartThreadOptions;
use lemurclaw_core::SteerInputError;
use lemurclaw_core::ThreadConfigSnapshot;
use lemurclaw_core::ThreadManager;
use lemurclaw_core::config::Config;
use lemurclaw_core::config::ConfigOverrides;
use lemurclaw_core::config::NetworkProxyAuditMetadata;
use lemurclaw_core::config::edit::ConfigEdit;
use lemurclaw_core::config::edit::ConfigEditsBuilder;
use lemurclaw_core::connectors::AccessibleConnectorsStatus;
use lemurclaw_core::exec::ExecCapturePolicy;
use lemurclaw_core::exec::ExecExpiration;
use lemurclaw_core::exec::ExecParams;
use lemurclaw_core::exec_env::create_env;
use lemurclaw_core::path_utils;
#[cfg(test)]
use lemurclaw_core::read_head_for_summary;
use lemurclaw_core::sandboxing::SandboxPermissions;
use lemurclaw_core::truncate_rollout_after_turn_id;
use lemurclaw_core::truncate_rollout_before_turn_id;
use lemurclaw_core::windows_sandbox::WindowsSandboxLevelExt;
use lemurclaw_core::windows_sandbox::WindowsSandboxSetupMode as CoreWindowsSandboxSetupMode;
use lemurclaw_core::windows_sandbox::WindowsSandboxSetupRequest;
use lemurclaw_core::windows_sandbox::sandbox_setup_is_complete;
use lemurclaw_core_plugins::PluginInstallError as CorePluginInstallError;
use lemurclaw_core_plugins::PluginInstallRequest;
use lemurclaw_core_plugins::PluginReadRequest;
use lemurclaw_core_plugins::PluginUninstallError as CorePluginUninstallError;
use lemurclaw_core_plugins::PluginsManager;
use lemurclaw_core_plugins::loader::load_plugin_apps;
use lemurclaw_core_plugins::manifest::PluginManifestInterface;
use lemurclaw_core_plugins::marketplace::MarketplaceError;
use lemurclaw_core_plugins::marketplace::MarketplacePluginSource;
use lemurclaw_core_plugins::marketplace_add::MarketplaceAddError;
use lemurclaw_core_plugins::marketplace_add::MarketplaceAddRequest;
use lemurclaw_core_plugins::marketplace_add::add_marketplace as add_marketplace_to_codex_home;
use lemurclaw_core_plugins::marketplace_remove::MarketplaceRemoveError;
use lemurclaw_core_plugins::marketplace_remove::MarketplaceRemoveRequest as CoreMarketplaceRemoveRequest;
use lemurclaw_core_plugins::marketplace_remove::remove_marketplace;
use lemurclaw_core_plugins::remote::RemoteMarketplace;
use lemurclaw_core_plugins::remote::RemoteMarketplaceSource;
use lemurclaw_core_plugins::remote::RemotePluginCatalogError;
use lemurclaw_core_plugins::remote::RemotePluginDetail as RemoteCatalogPluginDetail;
use lemurclaw_core_plugins::remote::RemotePluginServiceConfig;
use lemurclaw_core_plugins::remote::RemotePluginShareContext as RemoteCatalogPluginShareContext;
use lemurclaw_core_plugins::remote::RemotePluginShareSummary as RemoteCatalogPluginShareSummary;
use lemurclaw_core_plugins::remote::RemotePluginSummary as RemoteCatalogPluginSummary;
use lemurclaw_exec_server::EnvironmentManager;
use lemurclaw_exec_server::EnvironmentObservedStatus;
use lemurclaw_exec_server::LOCAL_ENVIRONMENT_ID;
use lemurclaw_exec_server::LOCAL_FS;
use lemurclaw_features::FEATURES;
use lemurclaw_features::Feature;
use lemurclaw_features::Stage;
use lemurclaw_feedback::CodexFeedback;
use lemurclaw_feedback::FeedbackAttachmentPath;
use lemurclaw_feedback::FeedbackUploadOptions;
use lemurclaw_git_utils::git_diff_to_remote;
use lemurclaw_git_utils::resolve_root_git_project_for_trust;
use lemurclaw_login::AuthManager;
use lemurclaw_login::CODEX_OPEN_APP_URL;
use lemurclaw_login::CodexAuth;
use lemurclaw_login::LoginSuccessPage;
use lemurclaw_login::LoginSuccessPageBrand;
use lemurclaw_login::ServerOptions as LoginServerOptions;
use lemurclaw_login::ShutdownHandle;
use lemurclaw_login::complete_device_code_login;
use lemurclaw_login::login_with_api_key;
use lemurclaw_login::login_with_bedrock_api_key;
use lemurclaw_login::oauth_client_id;
use lemurclaw_login::request_device_code;
use lemurclaw_login::run_login_server;
use lemurclaw_mcp::McpRuntimeContext;
use lemurclaw_mcp::McpServerStatusSnapshot;
use lemurclaw_mcp::McpSnapshotDetail;
use lemurclaw_mcp::collect_mcp_server_status_snapshot_with_detail;
use lemurclaw_mcp::discover_supported_scopes;
use lemurclaw_mcp::read_mcp_resource as read_mcp_resource_without_thread;
use lemurclaw_mcp::resolve_oauth_scopes;
use lemurclaw_memories_write::clear_memory_roots_contents;
use lemurclaw_model_provider::create_model_provider;
use lemurclaw_models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;
use lemurclaw_protocol::ThreadId;
use lemurclaw_protocol::config_types::CollaborationMode;
use lemurclaw_protocol::config_types::ForcedLoginMethod;
use lemurclaw_protocol::config_types::Personality;
use lemurclaw_protocol::config_types::ReasoningSummary;
use lemurclaw_protocol::config_types::TrustLevel;
use lemurclaw_protocol::config_types::WindowsSandboxLevel;
use lemurclaw_protocol::error::CodexErr;
use lemurclaw_protocol::error::Result as CodexResult;
#[cfg(test)]
use lemurclaw_protocol::items::TurnItem;
use lemurclaw_protocol::models::ResponseItem;
use lemurclaw_protocol::openai_models::ReasoningEffort;
#[cfg(test)]
use lemurclaw_protocol::permissions::FileSystemSandboxPolicy;
use lemurclaw_protocol::protocol::AgentStatus;
use lemurclaw_protocol::protocol::ConversationAudioParams;
use lemurclaw_protocol::protocol::ConversationSpeechParams;
use lemurclaw_protocol::protocol::ConversationStartParams;
use lemurclaw_protocol::protocol::ConversationStartTransport;
use lemurclaw_protocol::protocol::ConversationTextParams;
use lemurclaw_protocol::protocol::EventMsg;
#[cfg(test)]
use lemurclaw_protocol::protocol::GitInfo as CoreGitInfo;
use lemurclaw_protocol::protocol::InitialHistory;
use lemurclaw_protocol::protocol::McpAuthStatus as CoreMcpAuthStatus;
use lemurclaw_protocol::protocol::Op;
use lemurclaw_protocol::protocol::RealtimeVoicesList;
use lemurclaw_protocol::protocol::ResumedHistory;
use lemurclaw_protocol::protocol::ReviewDelivery as CoreReviewDelivery;
use lemurclaw_protocol::protocol::ReviewRequest;
use lemurclaw_protocol::protocol::ReviewTarget as CoreReviewTarget;
use lemurclaw_protocol::protocol::RolloutItem;
use lemurclaw_protocol::protocol::SessionConfiguredEvent;
#[cfg(test)]
use lemurclaw_protocol::protocol::SessionMetaLine;
use lemurclaw_protocol::protocol::TurnEnvironmentSelection;
use lemurclaw_protocol::protocol::TurnEnvironmentSelections;
use lemurclaw_protocol::protocol::W3cTraceContext;
use lemurclaw_protocol::protocol::strip_user_message_prefix;
use lemurclaw_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use lemurclaw_protocol::user_input::UserInput as CoreInputItem;
use lemurclaw_rmcp_client::perform_oauth_login_return_url;
use lemurclaw_rollout::is_persisted_rollout_item;
use lemurclaw_rollout::state_db::StateDbHandle;
use lemurclaw_rollout::state_db::reconcile_rollout;
use lemurclaw_state::ThreadMetadata;
use lemurclaw_state::log_db::LogDbLayer;
use lemurclaw_thread_store::ArchiveThreadParams as StoreArchiveThreadParams;
use lemurclaw_thread_store::ArchiveThreadsParams as StoreArchiveThreadsParams;
use lemurclaw_thread_store::DeleteThreadsParams as StoreDeleteThreadsParams;
use lemurclaw_thread_store::GitInfoPatch as StoreGitInfoPatch;
use lemurclaw_thread_store::ItemSortKey as StoreItemSortKey;
use lemurclaw_thread_store::ListItemsParams as StoreListItemsParams;
use lemurclaw_thread_store::ListThreadsParams as StoreListThreadsParams;
use lemurclaw_thread_store::ListTurnsParams as StoreListTurnsParams;
use lemurclaw_thread_store::LoadThreadHistoryParams as StoreLoadThreadHistoryParams;
use lemurclaw_thread_store::LocalThreadStore;
use lemurclaw_thread_store::ReadThreadByRolloutPathParams as StoreReadThreadByRolloutPathParams;
use lemurclaw_thread_store::ReadThreadParams as StoreReadThreadParams;
use lemurclaw_thread_store::SearchThreadOccurrencesParams as StoreSearchThreadOccurrencesParams;
use lemurclaw_thread_store::SearchThreadsParams as StoreSearchThreadsParams;
use lemurclaw_thread_store::SortDirection as StoreSortDirection;
use lemurclaw_thread_store::StoredThread;
use lemurclaw_thread_store::StoredTurn;
use lemurclaw_thread_store::StoredTurnItemsView;
use lemurclaw_thread_store::StoredTurnStatus;
use lemurclaw_thread_store::ThreadMetadataPatch as StoreThreadMetadataPatch;
use lemurclaw_thread_store::ThreadRelationFilter as StoreThreadRelationFilter;
use lemurclaw_thread_store::ThreadSortKey as StoreThreadSortKey;
use lemurclaw_thread_store::ThreadStore;
use lemurclaw_thread_store::ThreadStoreError;
use lemurclaw_utils_absolute_path::AbsolutePathBuf;
use lemurclaw_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;
use tokio_util::task::TaskTracker;
use toml::Value as TomlValue;
use tracing::Instrument;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

#[cfg(test)]
use lemurclaw_app_server_protocol::ServerRequest;

mod account_processor;
mod apps_processor;
mod bedrock_auth;
mod catalog_processor;
mod command_exec_processor;
mod config_processor;
mod environment_processor;
mod feedback_doctor_report;
mod feedback_processor;
mod fs_processor;
mod git_processor;
mod initialize_processor;
mod marketplace_processor;
mod mcp_processor;
mod plugins;
mod process_exec_processor;
mod remote_control_processor;
mod search;
mod thread_enrichment;
mod thread_fork_goal;
mod thread_processor;
mod token_usage_replay;
mod turn_processor;
mod windows_sandbox_processor;

pub(crate) use account_processor::AccountRequestProcessor;
pub(crate) use apps_processor::AppsRequestProcessor;
pub(crate) use catalog_processor::CatalogRequestProcessor;
pub(crate) use command_exec_processor::CommandExecRequestProcessor;
pub(crate) use config_processor::ConfigRequestProcessor;
pub(crate) use environment_processor::EnvironmentRequestProcessor;
pub(crate) use feedback_processor::FeedbackRequestProcessor;
pub(crate) use fs_processor::FsRequestProcessor;
pub(crate) use git_processor::GitRequestProcessor;
pub(crate) use initialize_processor::InitializeRequestProcessor;
pub(crate) use marketplace_processor::MarketplaceRequestProcessor;
pub(crate) use mcp_processor::McpRequestProcessor;
pub(crate) use plugins::PluginRequestProcessor;
pub(crate) use process_exec_processor::ProcessExecRequestProcessor;
pub(crate) use remote_control_processor::RemoteControlRequestProcessor;
pub(crate) use search::SearchRequestProcessor;
pub(crate) use thread_goal_processor::ThreadGoalRequestProcessor;
pub(crate) use thread_processor::ThreadRequestProcessor;
pub(crate) use turn_processor::TurnRequestProcessor;
pub(crate) use windows_sandbox_processor::WindowsSandboxRequestProcessor;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::filters::compute_source_filters;
use crate::filters::source_kind_matches;
use crate::thread_state::ConnectionCapabilities;
use crate::thread_state::ThreadListenerCommand;
use crate::thread_state::ThreadState;
use crate::thread_state::ThreadStateManager;
use token_usage_replay::restored_token_usage_turn_id;
use token_usage_replay::send_thread_token_usage_update_to_connection;

fn resolve_request_cwd(cwd: Option<PathBuf>) -> Result<Option<AbsolutePathBuf>, JSONRPCErrorError> {
    cwd.map(|cwd| {
        AbsolutePathBuf::relative_to_current_dir(path_utils::normalize_for_native_workdir(cwd))
            .map_err(|err| invalid_request(format!("invalid cwd: {err}")))
    })
    .transpose()
}

fn resolve_turn_environment_selections(
    thread_manager: &ThreadManager,
    environments: Option<Vec<TurnEnvironmentParams>>,
) -> Result<Option<Vec<TurnEnvironmentSelection>>, JSONRPCErrorError> {
    let Some(environments) = environments else {
        return Ok(None);
    };
    let mut selections = Vec::with_capacity(environments.len());
    for environment in environments {
        let environment_id = environment.environment_id;
        let cwd = environment
            .cwd
            .to_inferred_path_uri()
            .ok_or_else(|| {
                invalid_request(format!(
                    "invalid cwd for environment `{environment_id}`: path `{}` does not use absolute POSIX or Windows path syntax",
                    environment.cwd
                ))
            })?;
        let workspace_roots = environment
            .runtime_workspace_roots
            .map(|roots| {
                let mut resolved_roots = Vec::new();
                for root in roots {
                    let root = root.to_inferred_path_uri().ok_or_else(|| {
                        invalid_request(format!(
                            "invalid runtime workspace root for environment `{environment_id}`: path `{root}` does not use absolute POSIX or Windows path syntax"
                        ))
                    })?;
                    if !resolved_roots.contains(&root) {
                        resolved_roots.push(root);
                    }
                }
                Ok::<_, JSONRPCErrorError>(resolved_roots)
            })
            .transpose()?
            .unwrap_or_else(|| vec![cwd.clone()]);
        selections.push(TurnEnvironmentSelection {
            environment_id,
            cwd,
            workspace_roots,
        });
    }
    thread_manager
        .validate_environment_selections(&selections)
        .map_err(environment_selection_error)?;
    Ok(Some(selections))
}

fn resolve_runtime_workspace_roots(workspace_roots: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    let mut resolved_roots = Vec::new();
    for root in workspace_roots {
        if !resolved_roots.iter().any(|existing| existing == &root) {
            resolved_roots.push(root);
        }
    }
    resolved_roots
}

mod config_errors;
mod request_errors;
mod thread_delete;
mod thread_goal_processor;
mod thread_lifecycle;
mod thread_resume_redaction;
mod thread_summary;

use self::config_errors::*;
use self::request_errors::*;
use self::thread_goal_processor::api_thread_goal_from_state;
use self::thread_lifecycle::*;
use self::thread_resume_redaction::*;
use self::thread_summary::*;

pub(crate) use self::thread_lifecycle::populate_thread_turns_from_history;
pub(crate) use self::thread_processor::thread_from_stored_thread;
#[cfg(test)]
pub(crate) use self::thread_summary::read_summary_from_rollout;
#[cfg(test)]
pub(crate) use self::thread_summary::summary_to_thread;
pub(crate) use self::thread_summary::thread_settings_from_config_snapshot;
pub(crate) use self::thread_summary::thread_settings_from_core_snapshot;

pub(crate) fn build_legacy_api_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut builder = ThreadHistoryBuilder::new();
    for item in items {
        if is_persisted_rollout_item(item, lemurclaw_protocol::protocol::ThreadHistoryMode::Legacy) {
            builder.handle_rollout_item(item);
        }
    }
    builder.finish()
}
