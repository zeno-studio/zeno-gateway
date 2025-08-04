use std::collections::HashMap;

pub fn setup_ankr_endpoints(rpc_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
    if ankr_key.is_empty() {
        println!("Warning: ANKR_API_KEY is empty, skipping Ankr endpoints");
        return;
    }

    let chains = vec![
        ("ankr_eth", "eth"),
        ("ankr_bsc", "bsc"),
        ("ankr_arbitrum", "arbitrum"),
        ("ankr_optimism", "optimism"),
        ("ankr_base", "base"),
        ("ankr_polygon", "polygon"),
    ];

    for (endpoint_name, chain) in chains {
        let url = format!("https://rpc.ankr.com/{}/{}", chain, ankr_key);
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

pub fn setup_blast_endpoints(rpc_endpoints: &mut HashMap<String, String>, blast_key: &str) {
    if blast_key.is_empty() {
        println!("Warning: BLAST_API_KEY is empty, skipping Blast endpoints");
        return;
    }

    let endpoints = vec![
        (
            "blast_eth",
            format!("https://eth-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_bsc",
            format!("https://bsc-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_arbitrum",
            format!("https://arbitrum-one.blastapi.io/{}", blast_key),
        ),
        (
            "blast_optimism",
            format!("https://optimism-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_base",
            format!("https://base-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_polygon",
            format!("https://polygon-mainnet.blastapi.io/{}", blast_key),
        ),
    ];

    for (endpoint_name, url) in endpoints {
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

pub fn setup_indexer_endpoints(indexer_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
    if ankr_key.is_empty() {
        println!("Warning: ANKR_API_KEY is empty, skipping indexer endpoints");
        return;
    }

    let endpoints = vec![
        (
            "ankr",
            format!("https://rpc.ankr.com/multichain/{}", ankr_key),
        ),
    ];

    for (endpoint_name, url) in endpoints {
        indexer_endpoints.insert(endpoint_name.to_string(), url);
    }
}
