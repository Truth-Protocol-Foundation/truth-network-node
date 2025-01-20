// Copyright 2024 Aventus Network Services.
// This file is part of Aventus.

// TNF specific cli configuration
use clap::Parser;

#[derive(Debug, Parser)]
pub struct TnfCliConfiguration {
    pub tnf_service_port: Option<String>,
    pub ethereum_node_url: Option<String>,
}
