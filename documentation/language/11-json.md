# 11. JSON Handling

JSON in XCX is an immutable data buffer with rigorous type validation. There are no `null` values — encountering one stops the program (HALT).

## 11.1. Import and Parsing

Import a JSON file as raw text, then parse it:

```xcx
import "data.json" as file_contents;
let (json) data_from_file = json.parse(file_contents);
```

Or parse a string directly:

```xcx
let (str) json_text = "{\"name\":\"John\",\"age\":25}";
let (json) data = json.parse(json_text);
```

## 11.2. Raw Blocks (`<<< >>>`)

Raw blocks let you write JSON naturally, without escaping quotes:

```xcx
let (json) clean_data <<<
{
    "user": {
        "name": "John",
        "last_name": "Smith",
        "preferences": {
            "theme": "dark",
            "notifications": true
        }
    },
    "settings": ["option1", "option2"]
}
>>>;
```

Compare with the traditional approach, which requires escaping every quote:

```xcx
# let (str) traditional_json = "{\"text\":\"This is a \\\"quote\\\"\"}";
```

## 11.3. Operations and Methods

```xcx
# Number of top-level elements:
>! "Number of elements: " + data.count();

# Check whether a path exists (dot notation):
>! "Path exists: " + data.exists("user.name");

# Serialization back to text:
let (str) as_text = data.to_str();
```

## 11.4. Binding JSON Data to Variables

`bind()` copies a value from a JSON path into an XCX variable. The variable's type must match the JSON value's type:

```xcx
let (str) user_name;
let (boolean) notifications;

data.bind("user.name", user_name);
data.bind("user.preferences.notifications", notifications);

>! "User: " + user_name;
>! "Notifications: " + notifications;
```

Type mismatches are errors:

```xcx
# let (float) wrong_type;
# data.bind("user.name", wrong_type);  # ERROR: Type mismatch
```

For optional fields, check existence first:

```xcx
if (data.exists("optional.field")) then;
    let (str) value;
    data.bind("optional.field", value);
els;
    >! "Optional field does not exist";
end;
```

## 11.5. Injecting Data into Structures

`inject()` maps JSON fields onto a target structure using a mapping map (`json_field` :: `target_field`):

```xcx
let (map :: str <-> str) mapping = {
    "id" :: "user_id",
    "name" :: "first_name"
};

data.inject(data.users, mapping, "target_table");
```
