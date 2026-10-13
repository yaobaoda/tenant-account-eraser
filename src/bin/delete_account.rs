use std::process::ExitCode;
use tenant_account_eraser::account_deletion::{
    AccountDeletionService, AccountState, DeletionRequest,
};
use tenant_account_eraser::infrai_client::InfraiClient;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("deletion did not complete: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let request = DeletionRequest {
        tenant_id: required_arg(&mut args, "tenant_id")?,
        user_id: required_arg(&mut args, "user_id")?,
        credential_id: required_arg(&mut args, "credential_id")?,
    };

    let service = AccountDeletionService::new(InfraiClient::from_env()?);
    let receipt = service.delete(request, AccountState::Active).await?;
    println!(
        "deleted user={} tenant={} revoked_sessions={} credential_revoked={}",
        receipt.user_id, receipt.tenant_id, receipt.revoked_sessions, receipt.credential_revoked
    );
    Ok(())
}

fn required_arg(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("missing {name}; expected: tenant_id user_id credential_id").into())
}
