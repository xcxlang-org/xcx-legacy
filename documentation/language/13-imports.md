# 13. Import System

## 13.1. Import Mechanism

```xcx
# Importing a single file:
import "utils.xcx";

# Import with alias:
import "library/dates.xcx" as date_functions;

# Importing JSON files:
import "config.json" as config_json;
```

## 13.2. Typical Project Structure

```
project/
├── main.xcx
├── config/
│   ├── database.xcx
│   └── settings.xcx
├── models/
│   ├── user.xcx
│   └── product.xcx
├── utils/
│   ├── string_utils.xcx
│   └── math_utils.xcx
└── data/
    └── sample.json
```

## 13.3. Dependency Management

Import files in dependency order — configuration and models before the code that uses them:

```xcx
import "config/settings.xcx";      # Configuration first
import "models/user.xcx";          # Models
import "utils/string_utils.xcx";   # Helper utilities
```

Avoid circular dependencies:

```
# File A.xcx -> import B.xcx
# File B.xcx -> import C.xcx
# File C.xcx -> import A.xcx   # CIRCULAR DEPENDENCY - do not do this
```
