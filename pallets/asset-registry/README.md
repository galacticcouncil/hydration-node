### Asset registry

## Overview
Asset registry provides functionality to create, store and keep tracking of existing assets in a system.

### Terminology

- **CoreAssetId** - asset id of native/core asset. Usually 0.
- **NextAssetId** - asset id to be assigned for next asset added to the system. Must be > CoreAssetId
- **AssetIds** - list of existing asset ids

### Interface
- `get_or_create_asset` - creates new asset id for give asset name. If such asset already exists, it returns the corresponding asset id.
