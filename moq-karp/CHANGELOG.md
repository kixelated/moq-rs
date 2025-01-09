# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.11.0...moq-karp-v0.11.1) - 2024-12-12

### Other

- Fix a race condition on subscribe ([#268](https://github.com/kixelated/moq-rs/pull/268))

## [0.11.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.10.0...moq-karp-v0.11.0) - 2024-12-04

### Other

- Bump thiserror from 1.0.69 to 2.0.3 ([#254](https://github.com/kixelated/moq-rs/pull/254))
- Bump bytes from 1.8.0 to 1.9.0 ([#253](https://github.com/kixelated/moq-rs/pull/253))
- Add support for immediate 404s ([#241](https://github.com/kixelated/moq-rs/pull/241))
- Npm fix ([#249](https://github.com/kixelated/moq-rs/pull/249))

## [0.10.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.9.0...moq-karp-v0.10.0) - 2024-11-26

### Other

- Karp cleanup and URL reshuffling ([#239](https://github.com/kixelated/moq-rs/pull/239))

## [0.9.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.8.1...moq-karp-v0.9.0) - 2024-11-23

### Other

- Refactor and flatten the Karp API ([#238](https://github.com/kixelated/moq-rs/pull/238))
- Fix the resumable broadcast detection for karp. ([#236](https://github.com/kixelated/moq-rs/pull/236))

## [0.8.1](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.8.0...moq-karp-v0.8.1) - 2024-11-10

### Other

- Remove default CLI feature.
- Fix a few constants. ([#231](https://github.com/kixelated/moq-rs/pull/231))

## [0.8.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.7.1...moq-karp-v0.8.0) - 2024-11-09

### Other

- Fix moq-karp audio. ([#228](https://github.com/kixelated/moq-rs/pull/228))

## [0.7.1](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.7.0...moq-karp-v0.7.1) - 2024-11-07

### Other

- Add some more/better logging. ([#227](https://github.com/kixelated/moq-rs/pull/227))
- Fix a bug with partial mdat chunks. ([#226](https://github.com/kixelated/moq-rs/pull/226))
- Auto upgrade dependencies with release-plz ([#224](https://github.com/kixelated/moq-rs/pull/224))

## [0.7.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.6.0...moq-karp-v0.7.0) - 2024-10-29

### Other

- Karp API improvements ([#220](https://github.com/kixelated/moq-rs/pull/220))

## [0.6.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.5.0...moq-karp-v0.6.0) - 2024-10-28

### Other

- Small API changes to avoid needing the Session upfront. ([#216](https://github.com/kixelated/moq-rs/pull/216))

## [0.5.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.4.2...moq-karp-v0.5.0) - 2024-10-28

### Other

- Add support for resumable broadcasts ([#213](https://github.com/kixelated/moq-rs/pull/213))

## [0.4.2](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.4.1...moq-karp-v0.4.2) - 2024-10-28

### Other

- Require --path argument ([#211](https://github.com/kixelated/moq-rs/pull/211))

## [0.4.1](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.4.0...moq-karp-v0.4.1) - 2024-10-28

### Other

- update Cargo.lock dependencies

## [0.4.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.3.0...moq-karp-v0.4.0) - 2024-10-27

### Other

- Remove broadcasts from moq-transfork; tracks have a path instead ([#204](https://github.com/kixelated/moq-rs/pull/204))
- Use a path instead of name for Broadcasts ([#200](https://github.com/kixelated/moq-rs/pull/200))
- Run moq-bbb in docker-compose. ([#201](https://github.com/kixelated/moq-rs/pull/201))

## [0.3.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.2.0...moq-karp-v0.3.0) - 2024-10-18

### Other

- Support closing broadcasts. ([#199](https://github.com/kixelated/moq-rs/pull/199))
- Upgrade MP4 ([#196](https://github.com/kixelated/moq-rs/pull/196))

## [0.2.0](https://github.com/kixelated/moq-rs/compare/moq-karp-v0.1.0...moq-karp-v0.2.0) - 2024-10-14

### Other

- Support parsing a byte buffer instead of AsyncReader. ([#193](https://github.com/kixelated/moq-rs/pull/193))
- Bump moq-native
- Transfork - Full rewrite  ([#191](https://github.com/kixelated/moq-rs/pull/191))
