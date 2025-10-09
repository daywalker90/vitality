# Changelog

## [0.2.3] - 2025-10-09

### Changed
- switched out jemalloc with mimalloc for better compatibility
- upgraded dependencies


## [0.2.2] - 2025-03-11

### Changed

- upgraded dependencies

## [0.2.1] - 2024-12-09

### Fixed

- CLN 24.11: false positive detection of unwilling-to-reconnect peers, vitality can't detect this in CLN 24.11 since the status never? shows the reconnect attempts

## [0.2.0] - 2024-09-23

### Added
- nix flake (thanks to @RCasatta)

### Changed
- updated dependencies

## [0.1.10] - 2024-06-05

### Changed

- Options code refactored. All options are now natively dynamic. Read the updated README section on how to set options for more information

## [0.1.9] - 2024-05-03

### Added

- Check for lost channel state in CLN v24.02+
