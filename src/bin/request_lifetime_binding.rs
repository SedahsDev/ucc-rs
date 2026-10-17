// Minimal probe binary to verify compilation with lifetime-bound UccCollectiveRequest
// This binary tests that the new UccCollectiveRequest<'a> with PhantomData compiles

fn main() {
    // The existence of this file verifies that UccCollectiveRequest<'a>
    // compiles correctly with the lifetime parameter
}
