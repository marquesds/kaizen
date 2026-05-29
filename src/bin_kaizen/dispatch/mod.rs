mod common;
mod improve;
mod improve_exp;
mod interchange;
mod operate;
mod operate_daemon;
mod operate_telemetry;
mod trust;
mod trust_artifacts;
mod trust_metrics;
mod trust_sessions;

use crate::bin_kaizen::args::*;

pub(crate) fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.cmd {
        Command::Daemon { subcmd } => operate_daemon::daemon(subcmd),
        Command::Ingest { subcmd } => operate::ingest(subcmd),
        Command::Gc {
            workspace,
            project,
            days,
            vacuum,
        } => operate::gc(workspace, project, days, vacuum),
        Command::Migrate { subcmd } => operate::migrate(subcmd),
        Command::Completions { shell } => operate::completions(shell),
        Command::Sync { subcmd } => operate::sync(subcmd),
        Command::Telemetry { subcmd } => operate_telemetry::telemetry(subcmd),
        Command::Export { subcmd } => interchange::export(subcmd),
        Command::Import { subcmd } => interchange::import(subcmd),
        Command::Verify { subcmd } => interchange::verify(subcmd),
        Command::Upgrade { from_source } => operate::upgrade(from_source),
        Command::Mcp => operate::mcp(),
        Command::Proxy { subcmd } => operate::proxy(subcmd),
        Command::SamplerRun {
            workspace,
            session,
            pid,
        } => operate::sampler_run(workspace, session, pid),
        Command::Sessions { subcmd } => trust_sessions::sessions(subcmd),
        Command::Search { subcmd } => trust_sessions::search(subcmd),
        Command::Query {
            expr,
            since,
            limit,
            json,
            workspace,
            project,
        } => trust_sessions::query(expr, since, limit, json, workspace, project),
        Command::Feedback { subcmd } => trust_sessions::feedback(subcmd),
        Command::Summary {
            workspace,
            project,
            all_workspaces,
            json,
            refresh,
            source,
        } => trust::summary(workspace, project, all_workspaces, json, refresh, source),
        Command::Tui { workspace, project } => trust::tui(workspace, project),
        Command::Init {
            workspace,
            project,
            deep,
        } => trust::init(workspace, project, deep),
        Command::Doctor { workspace, project } => trust::doctor(workspace, project),
        Command::Load {
            workspace,
            project,
            json,
        } => trust::load(workspace, project, json),
        Command::Insights {
            workspace,
            project,
            all_workspaces,
            refresh,
            source,
        } => trust::insights(workspace, project, all_workspaces, refresh, source),
        Command::Guidance {
            subcmd,
            days,
            json,
            workspace,
            project,
            refresh,
            source,
        } => trust::guidance(subcmd, days, json, workspace, project, refresh, source),
        Command::Metrics {
            subcmd,
            days,
            json,
            force,
            workspace,
            project,
            all_workspaces,
            refresh,
            source,
        } => trust_metrics::metrics(trust_metrics::MetricsRequest {
            subcmd,
            report: trust_metrics::MetricsReportRequest {
                days,
                json,
                force,
                workspace,
                project,
                all_workspaces,
                refresh,
                source,
            },
        }),
        Command::Observe {
            agent,
            workspace,
            project,
            command,
        } => trust::observe(agent, workspace, project, command),
        Command::Projects { subcmd } => trust::projects(subcmd),
        Command::Prompt { subcmd } => trust_artifacts::prompt(subcmd),
        Command::Outcomes { subcmd } => trust_artifacts::outcomes(subcmd),
        Command::Exp { subcmd } => improve_exp::exp(subcmd),
        Command::Cases { subcmd } => improve::cases(subcmd),
        Command::Rules { subcmd } => improve::rules(subcmd),
        Command::Alerts { subcmd } => improve::alerts(subcmd),
        Command::Review { subcmd } => improve::review(subcmd),
        Command::Eval { subcmd } => improve::eval(subcmd),
        Command::Retro {
            days,
            dry_run,
            json,
            force,
            workspace,
            project,
            refresh,
            source,
        } => improve::retro(improve::RetroRequest {
            days,
            dry_run,
            json,
            force,
            workspace,
            project,
            refresh,
            source,
        }),
    }
}
