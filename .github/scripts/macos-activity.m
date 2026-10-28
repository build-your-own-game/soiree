/* Controlled diagnostic injection only: Foundation is not added to a product.
 * https://developer.apple.com/documentation/foundation/processinfo/activityoptions/userinitiatedallowingidlesystemsleep
 * https://developer.apple.com/documentation/foundation/processinfo/activityoptions/latencycritical
 * The token lives for this test process; ending it is recorded at normal exit.
 */
#import <Foundation/Foundation.h>
#include <errno.h>
#include <mach/mach.h>
#include <mach/task_policy.h>
#include <pthread/qos.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>

static NSProcessInfo *process_info;
static id activity;
static NSActivityOptions options;
static const char *mode;

static void report_policy(const char *phase) {
    errno = 0;
    int process_bg = getpriority(PRIO_DARWIN_PROCESS, 0);
    int process_error = errno;
    errno = 0;
    int thread_bg = getpriority(PRIO_DARWIN_THREAD, 0);
    int thread_error = errno;
    task_category_policy_data_t category = {0};
    mach_msg_type_number_t count = TASK_CATEGORY_POLICY_COUNT;
    boolean_t defaulted = FALSE;
    kern_return_t role_status = task_policy_get(mach_task_self(),
        TASK_CATEGORY_POLICY, (task_policy_t)&category, &count, &defaulted);
    fprintf(stderr,
        "SOIREE_MACOS_POLICY {\"phase\":\"%s\",\"pid\":%d,\"ppid\":%d,\"pgid\":%d,"
        "\"main_thread_requested_qos\":%u,\"process_background\":%d,"
        "\"process_background_errno\":%d,\"main_thread_background\":%d,"
        "\"main_thread_background_errno\":%d,\"task_role\":%d,\"task_role_status\":%d}\n",
        phase, getpid(), getppid(), getpgrp(), qos_class_self(), process_bg,
        process_error, thread_bg, thread_error, category.role, role_status);
}

static void report_activity(const char *event) {
    fprintf(stderr,
        "SOIREE_MACOS_ACTIVITY {\"event\":\"%s\",\"pid\":%d,\"mode\":\"%s\","
        "\"options_raw\":%llu,\"token_created\":%s}\n",
        event, getpid(), mode, (unsigned long long)options,
        activity != nil ? "true" : "false");
}

__attribute__((constructor)) static void begin_controlled_activity(void) {
    mode = getenv("SOIREE_MACOS_CONTROLLED_ACTIVITY");
    if (mode == NULL || (strcmp(mode, "observe") != 0
            && strcmp(mode, "user-initiated-allow-sleep") != 0
            && strcmp(mode, "latency-critical") != 0)) {
        fputs("diagnostic activity requires an explicit controlled case\n", stderr);
        _exit(86);
    }
    @autoreleasepool {
        @try {
            process_info = [NSProcessInfo processInfo];
            report_policy("before-activity");
            if (strcmp(mode, "observe") == 0) {
                report_activity("observe");
                return;
            }
            options = NSActivityUserInitiatedAllowingIdleSystemSleep;
            if (strcmp(mode, "latency-critical") == 0) options |= NSActivityLatencyCritical;
            activity = [process_info beginActivityWithOptions:options
                reason:@"Isolated host timer-precision diagnostic"];
            report_activity("begin");
            if (activity == nil) _exit(87);
            report_policy("after-activity");
        } @catch (NSException *exception) {
            fprintf(stderr, "diagnostic activity failed: %s\n", exception.name.UTF8String);
            _exit(87);
        }
    }
}

__attribute__((destructor)) static void end_controlled_activity(void) {
    if (activity == nil) return;
    @autoreleasepool {
        @try {
            [process_info endActivity:activity];
            report_activity("end");
            activity = nil;
        } @catch (NSException *exception) {
            fprintf(stderr, "diagnostic endActivity failed: %s\n", exception.name.UTF8String);
            _exit(87);
        }
    }
}
