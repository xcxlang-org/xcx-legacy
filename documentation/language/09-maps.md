# 9. Key-Value Maps

Maps are strongly typed key-value structures. A map has no `null` values — reading a missing key stops the program (HALT).

## 9.1. Declaration and Initialization

Syntax: `let (map :: KeyType <-> ValueType) name = { key :: value, ... };`

```xcx
let (map :: str <-> int) person_ages = {
    "John" :: 25,
    "Anna" :: 30,
    "Peter" :: 35
};

let (map :: int <-> str) status_codes = {
    200 :: "OK",
    404 :: "Not found",
    500 :: "Server error"
};

let (map :: str <-> boolean) settings = {
    "debug" :: true,
    "logging" :: false,
    "notifications" :: true
};

# Empty map:
let (map :: int <-> float) measurements = {};
```

## 9.2. Methods and Operations

```xcx
# Adding / updating (existing key is overwritten):
person_ages.insert("Mary", 28);
person_ages.insert("John", 26);

# Getting values:
let (int) johns_age = person_ages.get("John");

# Checking key existence:
let (boolean) exists = person_ages.has("Anna");   # true
let (boolean) no_key = person_ages.has("Tommy");  # false

# Map size:
let (int) people_count = person_ages.size();

# Displaying contents:
person_ages.show();
```

## 9.3. Iteration

```xcx
for (str) person in person_ages.keys();
    let (int) age = person_ages.get(person);
    >! person + " is " + age + " years old";
end;
```

Iteration with filtering:

```xcx
for (str) key in settings.keys();
    let (boolean) value = settings.get(key);
    if (value == true) then;
        >! "Enabled: " + key;
    end;
end;
```

## 9.4. Maps as Constants

A constant map allows reads but blocks modifications:

```xcx
const (map :: str <-> int) configuration = {
    "port" :: 8080,
    "max_connections" :: 100
};

>! "Port: " + configuration.get("port");   # OK
>! "Size: " + configuration.size();        # OK

# configuration.insert("timeout", 30);     # ERROR: Cannot modify a constant
```
