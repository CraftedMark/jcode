#ifndef JCODE_MOBILE_FFI_H
#define JCODE_MOBILE_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MobileAppHandle MobileAppHandle;

MobileAppHandle *jcode_mobile_app_new(const char *initial_json);
void jcode_mobile_app_free(MobileAppHandle *app);

char *jcode_mobile_dispatch(MobileAppHandle *app, const char *action_json);
char *jcode_mobile_state(const MobileAppHandle *app);
char *jcode_mobile_tree(const MobileAppHandle *app);
char *jcode_mobile_logs(const MobileAppHandle *app, uint32_t limit);

void jcode_mobile_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
