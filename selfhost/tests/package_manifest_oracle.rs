#[path = "canonical_manifest.rs"] mod manifest;
#[path = "canonical_content.rs"] mod content;
#[path = "canonical_metadata.rs"] mod registry_metadata;
use serde_json::json;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 3 && (args[1] == "--semver" || args[1] == "--req") {
        let ok = if args[1] == "--semver" { semver::Version::parse(&args[2]).is_ok() }
                 else { semver::VersionReq::parse(&args[2]).is_ok() };
        println!("{}", json!({"ok":ok})); return;
    }
    if args.len() == 3 && args[1] == "--metadata" {
        let source = std::fs::read(&args[2]).unwrap();
        match serde_json::from_slice::<registry_metadata::RegistryPackageMetadata>(&source) {
            Ok(value) => {
                let bytes = serde_json::to_vec(&value).unwrap();
                let encoded_hex = bytes.iter().map(|byte|format!("{byte:02x}")).collect::<String>();
                println!("{}",json!({"ok":true,"value":value,"encoded_hex":encoded_hex}));
            }
            Err(error) => println!("{}",json!({"ok":false,"error":error.to_string()})),
        } return;
    }
    if args.len() == 3 && args[1] == "--content-checksum" {
        match content::checksum(std::path::Path::new(&args[2])) {
            Ok(sha256) => println!("{}",json!({"ok":true,"sha256":sha256})),
            Err(error) => println!("{}",json!({"ok":false,"error":error.to_string()})),
        } return;
    }
    for (id,path) in args[1..].iter().enumerate() {
        let source = std::fs::read_to_string(path).unwrap();
        let result = match manifest::Manifest::parse(&source) {
            Err(error) => json!({"id":id,"ok":false,"error":error.to_string()}),
            Ok(value) => {
                let dependencies: serde_json::Map<_,_> = value.dependencies.iter().map(|(key,dep)|
                    (key.clone(),json!({"name":dep.name,"package":dep.package,"path":dep.path.as_ref().map(|p|p.to_string_lossy()),"git":dep.git,"rev":dep.rev,"version_req":dep.version_req,"registry":dep.registry}))).collect();
                let registries: serde_json::Map<_,_> = value.registries.iter().map(|(key,item)|
                    (key.clone(),json!({"path":item.path.as_ref().map(|p|p.to_string_lossy()),"url":item.url,"token_env":item.token_env}))).collect();
                json!({"id":id,"ok":true,"manifest":{
                    "package":{"name":value.package.name,"version":value.package.version,"authors":value.package.authors,"description":value.package.description,"edition":value.package.edition},
                    "bin":value.bin.as_ref().map(|item|json!({"name":item.name,"path":item.path})),
                    "lib":value.lib.as_ref().map(|item|json!({"path":item.path})),
                    "test":value.test.iter().map(|item|json!({"path":item.path})).collect::<Vec<_>>(),
                    "registries":registries,"dependencies":dependencies}})
            }
        };
        println!("{}",result);
    }
}
