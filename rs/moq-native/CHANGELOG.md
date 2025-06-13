# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.1](https://github.com/kixelated/moq/compare/moq-native-v0.7.0...moq-native-v0.7.1) - 2025-06-13

### Fixed

- args for tls generate need to be without the port number ([#413](https://github.com/kixelated/moq/pull/413))

### Other

- Default to the first certificate when SNI matching fails. ([#414](https://github.com/kixelated/moq/pull/414))

## [0.7.0](https://github.com/kixelated/moq/compare/moq-native-v0.6.9...moq-native-v0.7.0) - 2025-06-03

### Other

- Add support for authentication tokens ([#399](https://github.com/kixelated/moq/pull/399))

## [0.6.9](https://github.com/kixelated/moq/compare/moq-native-v0.6.8...moq-native-v0.6.9) - 2025-05-21

### Other

- Split into Rust/Javascript halves and rebrand as moq-lite/hang ([#376](https://github.com/kixelated/moq/pull/376))

## [0.6.8](https://github.com/kixelated/moq/compare/moq-native-v0.6.7...moq-native-v0.6.8) - 2025-03-09

### Other

- Less aggressive idle timeout. ([#351](https://github.com/kixelated/moq/pull/351))

## [0.6.7](https://github.com/kixelated/moq/compare/moq-native-v0.6.6...moq-native-v0.6.7) - 2025-03-01

### Other

- updated the following local packages: moq-transfork

## [0.6.6](https://github.com/kixelated/moq/compare/moq-native-v0.6.5...moq-native-v0.6.6) - 2025-02-13

### Other

- Have moq-native return web_transport_quinn. ([#331](https://github.com/kixelated/moq/pull/331))

## [0.6.5](https://github.com/kixelated/moq/compare/moq-native-v0.6.4...moq-native-v0.6.5) - 2025-01-30

### Other

- Plane UI work ([#316](https://github.com/kixelated/moq/pull/316))

## [0.6.4](https://github.com/kixelated/moq/compare/moq-native-v0.6.3...moq-native-v0.6.4) - 2025-01-24

### Other

- updated the following local packages: moq-transfork

## [0.6.3](https://github.com/kixelated/moq/compare/moq-native-v0.6.2...moq-native-v0.6.3) - 2025-01-16

### Other

- Remove the useless openssl dependency. ([#295](https://github.com/kixelated/moq/pull/295))

## [0.6.2](https://github.com/kixelated/moq/compare/moq-native-v0.6.1...moq-native-v0.6.2) - 2025-01-16

### Other

- Retry connections to cluster nodes ([#290](https://github.com/kixelated/moq/pull/290))
- Switch to aws_lc_rs ([#287](https://github.com/kixelated/moq/pull/287))
- Support fetching fingerprint via native clients. ([#286](https://github.com/kixelated/moq/pull/286))
- Initial WASM contribute ([#283](https://github.com/kixelated/moq/pull/283))

## [0.6.1](https://github.com/kixelated/moq/compare/moq-native-v0.6.0...moq-native-v0.6.1) - 2025-01-13

### Other

- update Cargo.lock dependencies

## [0.6.0](https://github.com/kixelated/moq/compare/moq-native-v0.5.10...moq-native-v0.6.0) - 2025-01-13

### Other

- Raise the keep-alive. ([#278](https://github.com/kixelated/moq/pull/278))
- Replace mkcert with rcgen* ([#273](https://github.com/kixelated/moq/pull/273))

## [0.5.10](https://github.com/kixelated/moq/compare/moq-native-v0.5.9...moq-native-v0.5.10) - 2024-12-12

### Other

- Add support for RUST_LOG again. ([#267](https://github.com/kixelated/moq/pull/267))

## [0.5.9](https://github.com/kixelated/moq/compare/moq-native-v0.5.8...moq-native-v0.5.9) - 2024-12-04

### Other

- Move moq-gst and moq-web into the workspace. ([#258](https://github.com/kixelated/moq/pull/258))

## [0.5.8](https://github.com/kixelated/moq/compare/moq-native-v0.5.7...moq-native-v0.5.8) - 2024-11-26

### Other

- updated the following local packages: moq-transfork

## [0.5.7](https://github.com/kixelated/moq/compare/moq-native-v0.5.6...moq-native-v0.5.7) - 2024-11-23

### Other

- updated the following local packages: moq-transfork

## [0.5.6](https://github.com/kixelated/moq/compare/moq-native-v0.5.5...moq-native-v0.5.6) - 2024-11-07

### Other

- Add some more/better logging. ([#227](https://github.com/kixelated/moq/pull/227))
- Auto upgrade dependencies with release-plz ([#224](https://github.com/kixelated/moq/pull/224))

## [0.5.5](https://github.com/kixelated/moq/compare/moq-native-v0.5.4...moq-native-v0.5.5) - 2024-10-29

### Other

- Karp API improvements ([#220](https://github.com/kixelated/moq/pull/220))

## [0.5.4](https://github.com/kixelated/moq/compare/moq-native-v0.5.3...moq-native-v0.5.4) - 2024-10-28

### Other

- updated the following local packages: moq-transfork

## [0.5.3](https://github.com/kixelated/moq/compare/moq-native-v0.5.2...moq-native-v0.5.3) - 2024-10-27

### Other

- update Cargo.toml dependencies

## [0.5.2](https://github.com/kixelated/moq/compare/moq-native-v0.5.1...moq-native-v0.5.2) - 2024-10-18

### Other

- updated the following local packages: moq-transfork

## [0.2.2](https://github.com/kixelated/moq/compare/moq-native-v0.2.1...moq-native-v0.2.2) - 2024-07-24

### Other
- Add sslkeylogfile envvar for debugging ([#173](https://github.com/kixelated/moq/pull/173))

## [0.2.1](https://github.com/kixelated/moq/compare/moq-native-v0.2.0...moq-native-v0.2.1) - 2024-06-03

### Other
- Revert "filter DNS query results to only include addresses that our quic endpoint can use ([#166](https://github.com/kixelated/moq/pull/166))"
- filter DNS query results to only include addresses that our quic endpoint can use ([#166](https://github.com/kixelated/moq/pull/166))
- Remove Cargo.lock from moq-transport
