#ifndef VOLT_IOS_BRIDGE_H
#define VOLT_IOS_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
#define VOLT_HOST_ABI_VERSION 1
uint32_t volt_host_abi_version(void);
// Versioned private host ABI: all calls occur on UIKit's main thread. Keep metal_layer
// alive until volt_destroy; font/json buffers need only outlive their call.
// Returned strings are borrowed (copy immediately); never free them in Swift.
// volt_event: 0 = error, 1 = presented/suspended, 2 = retry surface acquisition.
uint64_t volt_create(void *metal_layer, const uint8_t *font, size_t font_len);
void volt_destroy(uint64_t handle);
int32_t volt_event(uint64_t handle, const uint8_t *json, size_t len);
const char *volt_snapshot(uint64_t handle);
const char *volt_error(void);
#endif
