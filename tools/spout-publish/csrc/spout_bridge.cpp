// spout_bridge.cpp — flat C wrapper around SpoutLibrary C++ interface
//
// Compile with: cl /LD spout_bridge.cpp /I<SpoutSDK> SpoutLibrary.lib
// Or use build.rs with the cc crate (see Cargo.toml).

#include "spout_bridge.h"
#include "SpoutLibrary.h"

extern "C" {

SpoutBridgeHandle spout_bridge_create(const char* name, unsigned int width, unsigned int height) {
    SPOUTHANDLE spout = GetSpout();
    if (!spout) return NULL;

    if (!spout->CreateSender(name, width, height)) {
        spout->Release();
        return NULL;
    }

    return (SpoutBridgeHandle)spout;
}

int spout_bridge_send_image(SpoutBridgeHandle h, const unsigned char* pixels,
                            unsigned int width, unsigned int height, int invert) {
    if (!h) return 0;
    SPOUTHANDLE spout = (SPOUTHANDLE)h;
    return spout->SendImage(pixels, width, height, GL_RGBA, invert != 0) ? 1 : 0;
}

void spout_bridge_release(SpoutBridgeHandle h) {
    if (!h) return;
    SPOUTHANDLE spout = (SPOUTHANDLE)h;
    spout->ReleaseSender();
    spout->Release();
}

} // extern "C"
