use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::Client as S3Client;
use tracing::info;

pub struct S3ClientBuilder;

impl S3ClientBuilder {
    pub async fn build(
        access_key_id: &str,
        secret_access_key: &str,
        region: &str,
        endpoint: Option<&str>,
    ) -> Result<S3Client> {
        let credentials = Credentials::new(access_key_id, secret_access_key, None, None, "eloqctl");

        let mut config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(Region::new(region.to_string()));

        if let Some(endpoint_url) = endpoint {
            config_builder = config_builder.endpoint_url(endpoint_url.to_string());
        }

        let config = config_builder.build();
        let client = S3Client::from_conf(config);
        Ok(client)
    }
}

pub async fn delete_s3_object(client: &S3Client, bucket: &str, key: &str) -> Result<()> {
    info!("Deleting S3 object: s3://{}/{}", bucket, key);

    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context(format!("Failed to delete s3://{}/{}", bucket, key))?;

    info!("Successfully deleted S3 object: s3://{}/{}", bucket, key);
    Ok(())
}
