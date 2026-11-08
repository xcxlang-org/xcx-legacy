# 12. Input / Output

## 12.1. Displaying Data (`>!`)

```xcx
>! "Hello world";
>! 42;
>! true;

let (str) name = "Anna";
let (int) age = 25;
>! "Name: " + name + ", age: " + age;
```

Complex objects display their structure:

```xcx
let (set) numbers { N = 1,,5 };
>! numbers;  # SetObject(SET_N: [1..5:1])
```

## 12.2. Reading Data (`>?`)

```xcx
>? user_name;
>! "Hello, " + user_name + "!";
```

Reading with validation:

```xcx
>! "Enter your age:";
>? user_age;

if (user_age >= 18) then;
    >! "You are an adult";
els;
    >! "You are a minor";
end;
```

## 12.3. Output Formatting

Floating point numbers are rounded automatically on display:

```xcx
let (float) price = 19.995678;
>! "Price: " + price;
```

Dates use `.format()`:

```xcx
let (date) today = date.now();
>! "Today is " + today.format("DD-MM-YYYY");
```

## 12.4. Data Storage Module (`store`)

The `store` module handles file operations. All paths are restricted to the project directory — path traversal attempts (e.g. `../`) stop the program.

```xcx
# Write / overwrite a file (UTF-8):
let (str) file = "data.txt";
store.write(file, "Sample content");

# Read a file (HALT if the file does not exist):
let (str) content = store.read(file);

# Append to the end of a file:
store.append(file, "\nAdditional line");

# Check existence:
if (store.exists(file)) then;
    >! "File exists";
els;
    >! "File does not exist";
end;
```

## 12.5. Terminal Commands

Terminal commands are executed with the `.terminal !command;` syntax:

```xcx
.terminal !clean;           # Clears the terminal screen
.terminal !exit;            # Ends the XCX interpreter
.terminal !help;            # Displays available commands
```
