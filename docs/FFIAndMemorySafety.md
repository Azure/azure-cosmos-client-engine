# FFI and Memory Safety in the C API

There are a few important things to keep in mind when building the C API, so that we can ensure it's "FFI-safe" (i.e. it follows the C ABI rules) and memory safe.

## FFI Safe Types

Every type passed in or out of C API functions must be FFI-safe.
This means that every value must be represented in a way that is compatible with C.
We break this down into several categories:

1. **Primitive types**: These are the basic types that are natively supported by C, such as `unsigned long`, `int`, `float`, etc.
   These types are directly compatible with C and can be passed in and out of C functions without any issues.
   Rust definitions of these types can be found in the [std::ffi module](https://doc.rust-lang.org/stable/std/ffi/index.html).
2. **Pointers**: These are raw pointers to memory that can be passed in and out of C functions.
   We have to be clear when accepting/returning pointers that we define the lifetime of the memory being pointed to.
   See below for a discussion of memory safety.
3. **Structures**: These are C-compatible structs that can be passed in and out of C functions.
   These structures must be defined in Rust using the `#[repr(C)]` attribute to ensure they are compatible with C.
   This also means they can only store **other** FFI-safe types as fields.
4. **Opaque types**: These are types that are not directly compatible with C, but can be passed through the FFI boundary via a pointer.
   The caller doesn't need to interact with these values themselves.
   They are returned by the engine as pointers and passed back in to the engine.
   For example, the query pipeline is such an opaque type.
   It is created by the `cosmoscx_v0_query_pipeline_create` function, which returns a pointer to the pipeline, and passed back in to the engine via the other functions in the pipeline API.
   Opaque types are usually implemented as a standard Rust struct, and then wrapped in a `Box<T>` to ensure they are heap-allocated and can be passed around safely (see below for more details about memory safety).

## FFI Results/Errors

When crossing the FFI boundary, we need to be careful to handle errors properly.
In Rust, we use the `Result<T, E>` type to represent a value that can either be a success or an error.
However, this type is not FFI-safe, so we need to convert it to a C-compatible type.

Instead, our C APIs return one of two types:

* `ResultCode` is a C-compatible signed integer enum (i.e. an `intptr_t`) that represents the result of a function call.
  It can be either `0` (success) or an error code (which is negative).
* `FfiResult<T>` is a C-compatible struct (though, you should also read "monomorphization" below) that holds a `ResultCode` and a `*const T` pointer pointing to the actual result value (which will be null if the result code indicates an error).

We also define a number of Rust conversions (i.e. the `::from` and `.into()` methods) to convert between standard Rust result types and the C-compatible types:

* Our `Error` type can be converted to a `ResultCode`
* A `Result<(), Error>` can be converted to a `ResultCode`, where `Ok(())` is converted to `ResultCode::Success` and `Err(e)` is converted to the error code.
* A `Result<Box<T>, Error>` can be converted to a `FfiResult<T>`, where `Ok(t)` is converted to a `FfiResult<T>` with a pointer to the value and `Err(e)` is converted to a `FfiResult<T>` with a null pointer and the error code.

Note that because we are returning pointers, it's only possible to convert from `Result<Box<T>, Error>` to `FfiResult<T>`, and not from `Result<T, Error>` to `FfiResult<T>`.
This is because an arbitrary `T` may not be FFI-safe, but a `Box<T>` can be converted to an opaque pointer type, which is FFI-safe.

## Monomorphization in the C API

"Monomorphization" is the process of converting generic types/functions into concrete types/functions at compile time.
A generic type/function is "polymorphic" because it can accept any type as a parameter.
Monomorphization takes such a polymorphic type/function and makes a single "monomorphic" (not parameterized) version of the type/function for each concrete type used in the code.
There are more details available in the [Rust documentation](https://doc.rust-lang.org/book/ch10-01-syntax.html#performance-of-code-using-generics), but to summarize, consider this type:

```rust
pub struct FfiResult<T> {
    pub result_code: ResultCode,
    pub value: *const T,
}
```

If you return this type from a function (whether it's a C-compatible function or not), the Rust compiler will generate a separate version of the struct depending on the type of `T` used.
For example:

```rust
fn foo() -> FfiResult<i32> { ... }
fn bar() -> FfiResult<i64> { ... }
```

This will generate two separate versions of the `FfiResult` struct, one for `i32` and one for `i64`.
The Rust compiler handles this automatically, and the generated code is optimized to be as efficient as possible.
Because the value is behind a pointer, the two versions of `FfiResult` are the same size and layout.
However, `cbindgen` (the tool we use to generate the C header file) still generates separate structs.

So, this C-compatible function:

```rust
pub unsafe extern "C" fn cosmoscx_v0_query_pipeline_create() -> FfiResult<Pipeline>;
```
Will generate a C header file that looks like this:

```c
typedef struct {
    ResultCode result_code;
    Pipeline* value;
} FfiResult_Pipeline;

FfiResult_Pipeline cosmoscx_v0_query_pipeline_create();
```

The `cbindgen` tool creates a separate C-compatible struct `FfiResult_Pipeline` representing an `FfiResult<Pipeline>` type.

This is all handled automatically by the `cbindgen` tool, but it's important to understand that this is how the C API works.

## Memory Safety

Building a C-compatible API in Rust has some major advantages.
We can leverage the memory safety of Rust to ensure memory that stays within Rust is properly freed and managed.
However, at the boundaries of the C API, those guarantees disappear and require additional management.

Every object (i.e. piece of memory) we return through the C API should have one of these lifetimes:

### Static memory

The object is global and exists for the lifetime of the process.
This corresponds to a `&'static` reference in Rust.
For example, the `cosmoscx_version` function returns a static string that is valid for the lifetime of the process.

### Owned memory

The object is created within the engine, and thus owned by the engine.
It must then be freed by the engine.
This corresponds to a `Box<T>` in Rust.
The `cosmoscx_v0_query_pipeline_create` function creates a pipeline object in _Rust's heap_ (as a `Box<Pipeline>`), and then "leaks" that Box (i.e. converts it to a raw pointer, thus hiding it from Rust's memory management) to return it to the caller.
The caller is then responsible for calling `cosmoscx_v0_query_pipeline_free` to free the memory when it is done with the object.
That function accepts the raw pointer and calls `Box::from_raw` to convert it back to a Box, thus making it "reappear" from Rust's perspective. The Box will then be freed when it goes out of scope.

### Borrowed memory

The object was created by the caller and is passed to the engine.
This means the Rust engine has no way of knowing the full lifetime of the object.
The only thing we can do is assert to the caller that the object must remain valid for some clearly defined lifetime.
In all current cases, we require the caller to keep the object valid for the duration of a specific function call.
For example, the `cosmoscx_v0_query_pipeline_create` function takes a `Str` value representing the JSON-serialized query plan itself.
The caller (Go, Python, etc.) owns that memory, so the Rust engine can only use it within the function call itself.
We implement this using lifetime parameters on the C functions:

```rust
pub unsafe extern "C" fn cosmoscx_v0_query_pipeline_create<'a>(query_plan: Str<'a>, ...) -> Pipeline;
```

A lifetime parameter is required here because the `Str` type contains a reference to the memory holding the string.
The lifetime parameter also serves to tell Rust that you can't store a `Str` value unless the object you're storing it in only lives as long as `'a`.
This has the effect of preventing us from storing the string in a struct like the `Pipeline` returned by this function (because it is not constrained with the `'a` lifetime parameter).
Instead, we are forced, by the Rust compiler, to use the `query_plan` value within the function itself (perhaps by cloning it into Rust-owned memory if needed).
This allows us to ensure the engine doesn't accidentally keep a reference to the memory that is owned by the caller.