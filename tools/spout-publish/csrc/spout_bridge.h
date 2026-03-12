// spout_bridge.h — flat C API wrapping SpoutLibrary's C++ vtable interface
//
// SpoutLibrary uses COM-style virtual methods on an opaque handle.
// This bridge exposes plain C functions callable via Rust FFI.
//
// Link against SpoutLibrary.lib or load SpoutLibrary.dll at runtime.

#ifndef SPOUT_BRIDGE_H
#define SPOUT_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

typedef void* SpoutBridgeHandle;

// Create a Spout sender. Returns an opaque handle, or NULL on failure.
SpoutBridgeHandle spout_bridge_create(const char* name, unsigned int width, unsigned int height);

// Send a raw RGBA pixel buffer. Returns 1 on success, 0 on failure.
int spout_bridge_send_image(SpoutBridgeHandle h, const unsigned char* pixels,
                            unsigned int width, unsigned int height, int invert);

// Release the sender and free resources.
void spout_bridge_release(SpoutBridgeHandle h);

#ifdef __cplusplus
}
#endif

#endif // SPOUT_BRIDGE_H
