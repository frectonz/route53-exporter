use std::fmt::Display;

use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
use aws_sdk_route53::{config::Region, meta::PKG_VERSION, types, Client};
use clap::Parser;
use color_eyre::Result;
use inquire::Select;
use tokio::fs;

use crate::export::HostedZoneExport;

/// Export Route53 Hosted Zones from a specific region.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The AWS Region
    #[arg(short, long)]
    region: Option<String>,

    /// The export filename
    #[arg(short, long, default_value = "route53-export.json")]
    export: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let Args { region, export } = Args::parse();

    let region = RegionProviderChain::first_try(region.map(Region::new))
        .or_default_provider()
        .or_else("us-east-1");
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .load()
        .await;

    println!("Route53 client version: {}", PKG_VERSION);
    let client = Client::new(&config);

    let hz = get_hosted_zone(&client).await?;

    let export_data: String = match hz {
        HZOption::All(hosted_zones) => {
            let mut exports = Vec::with_capacity(hosted_zones.len());
            for hz in hosted_zones {
                exports.push(get_export_data(&client, hz).await?);
            }
            serde_json::to_string_pretty(&exports)?
        }
        HZOption::HZ(hz) => {
            let export = get_export_data(&client, hz).await?;
            serde_json::to_string_pretty(&export)?
        }
    };

    fs::write(&export, export_data).await?;
    println!("Successfully exported data to {export} 🎉");

    Ok(())
}

#[derive(Debug, Clone)]
enum HZOption {
    All(Vec<types::HostedZone>),
    HZ(types::HostedZone),
}

impl Display for HZOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HZOption::All(_) => write!(f, "All"),
            HZOption::HZ(hz) => write!(f, "{}", hz.name()),
        }
    }
}

async fn get_hosted_zone(client: &aws_sdk_route53::Client) -> Result<HZOption> {
    let hosted_zone_count = client.get_hosted_zone_count().send().await?;

    println!(
        "Number of hosted zones in region : {}",
        hosted_zone_count.hosted_zone_count(),
    );

    let hosted_zones = client.list_hosted_zones().send().await?;
    let hosted_zones = hosted_zones.hosted_zones().to_owned();

    let mut options: Vec<HZOption> = hosted_zones.iter().cloned().map(HZOption::HZ).collect();
    options.insert(0, HZOption::All(hosted_zones));

    let hosted_zone = Select::new(
        "Choose the Hosted Zone you want to export.",
        options.to_owned(),
    )
    .prompt()?;

    Ok(hosted_zone)
}

async fn get_export_data(
    client: &aws_sdk_route53::Client,
    hz: types::HostedZone,
) -> Result<HostedZoneExport> {
    let records = client
        .list_resource_record_sets()
        .hosted_zone_id(hz.id())
        .send()
        .await?;

    Ok(HostedZoneExport::new(
        hz.id,
        hz.name,
        records.resource_record_sets,
    ))
}

mod export {
    use aws_sdk_route53::types;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct HostedZoneExport {
        id: String,
        name: String,
        record_sets: Vec<ResourceRecordSet>,
    }

    impl HostedZoneExport {
        pub fn new(id: String, name: String, record_sets: Vec<types::ResourceRecordSet>) -> Self {
            Self {
                id,
                name,
                record_sets: record_sets.into_iter().map(Into::into).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct ResourceRecordSet {
        name: String,
        r#type: String,
        set_identifier: Option<String>,
        weight: Option<i64>,
        region: Option<String>,
        geo_location: Option<GeoLocation>,
        failover: Option<String>,
        multi_value_answer: Option<bool>,
        ttl: Option<i64>,
        resource_records: Option<Vec<ResourceRecord>>,
        alias_target: Option<AliasTarget>,
        health_check_id: Option<String>,
        traffic_policy_instance_id: Option<String>,
        cidr_routing_config: Option<CidrRoutingConfig>,
        geo_proximity_location: Option<GeoProximityLocation>,
    }
    impl From<types::ResourceRecordSet> for ResourceRecordSet {
        fn from(value: types::ResourceRecordSet) -> Self {
            Self {
                name: value.name,
                r#type: value.r#type.as_str().to_owned(),
                set_identifier: value.set_identifier,
                weight: value.weight,
                region: value.region.map(|r| r.as_str().to_owned()),
                geo_location: value.geo_location.map(Into::into),
                failover: value.failover.map(|f| f.as_str().to_owned()),
                multi_value_answer: value.multi_value_answer,
                ttl: value.ttl,
                resource_records: value
                    .resource_records
                    .map(|records| records.into_iter().map(Into::into).collect()),
                alias_target: value.alias_target.map(Into::into),
                health_check_id: value.health_check_id,
                traffic_policy_instance_id: value.traffic_policy_instance_id,
                cidr_routing_config: value.cidr_routing_config.map(Into::into),
                geo_proximity_location: value.geo_proximity_location.map(Into::into),
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct GeoLocation {
        continent_code: Option<String>,
        country_code: Option<String>,
        subdivision_code: Option<String>,
    }
    impl From<types::GeoLocation> for GeoLocation {
        fn from(value: types::GeoLocation) -> Self {
            Self {
                continent_code: value.continent_code,
                country_code: value.country_code,
                subdivision_code: value.subdivision_code,
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct ResourceRecord {
        value: String,
    }
    impl From<types::ResourceRecord> for ResourceRecord {
        fn from(value: types::ResourceRecord) -> Self {
            Self { value: value.value }
        }
    }

    #[derive(Debug, Serialize)]
    struct AliasTarget {
        hosted_zone_id: String,
        dns_name: String,
        evaluate_target_health: bool,
    }
    impl From<types::AliasTarget> for AliasTarget {
        fn from(value: types::AliasTarget) -> Self {
            Self {
                hosted_zone_id: value.hosted_zone_id,
                dns_name: value.dns_name,
                evaluate_target_health: value.evaluate_target_health,
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct CidrRoutingConfig {
        collection_id: String,
        location_name: String,
    }
    impl From<types::CidrRoutingConfig> for CidrRoutingConfig {
        fn from(value: types::CidrRoutingConfig) -> Self {
            Self {
                collection_id: value.collection_id,
                location_name: value.location_name,
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct GeoProximityLocation {
        aws_region: Option<String>,
        local_zone_group: Option<String>,
        coordinates: Option<Coordinates>,
        bias: Option<i32>,
    }
    impl From<types::GeoProximityLocation> for GeoProximityLocation {
        fn from(value: types::GeoProximityLocation) -> Self {
            Self {
                aws_region: value.aws_region,
                local_zone_group: value.local_zone_group,
                coordinates: value.coordinates.map(Into::into),
                bias: value.bias,
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct Coordinates {
        latitude: String,
        longitude: String,
    }
    impl From<types::Coordinates> for Coordinates {
        fn from(value: types::Coordinates) -> Self {
            Self {
                latitude: value.latitude,
                longitude: value.longitude,
            }
        }
    }
}
