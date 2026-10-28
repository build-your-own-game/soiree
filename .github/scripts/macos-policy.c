/* Diagnostic injection only: never linked into a shipped host. It reads the
 * actual test process before libtest starts; it changes no policy or timer.
 */
#include <errno.h>
#include <mach/mach.h>
#include <mach/task_policy.h>
#include <pthread/qos.h>
#include <stdio.h>
#include <sys/resource.h>
#include <unistd.h>

__attribute__((constructor)) static void report_policy(void) {
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
    integer_t latency = 0;
    count = 1;
    defaulted = FALSE;
    kern_return_t latency_status = task_policy_get(mach_task_self(),
        TASK_BASE_LATENCY_QOS_POLICY, &latency, &count, &defaulted);
    integer_t throughput = 0;
    count = 1;
    defaulted = FALSE;
    kern_return_t throughput_status = task_policy_get(mach_task_self(),
        TASK_BASE_THROUGHPUT_QOS_POLICY, &throughput, &count, &defaulted);
    fprintf(stderr,
        "SOIREE_MACOS_POLICY {\"pid\":%d,\"ppid\":%d,\"pgid\":%d,"
        "\"main_thread_requested_qos\":%u,\"process_background\":%d,"
        "\"process_background_errno\":%d,\"main_thread_background\":%d,"
        "\"main_thread_background_errno\":%d,\"task_role\":%d,"
        "\"task_role_status\":%d,\"base_latency_raw\":%d,"
        "\"base_latency_status\":%d,\"base_throughput_raw\":%d,"
        "\"base_throughput_status\":%d}\n",
        getpid(), getppid(), getpgrp(), qos_class_self(), process_bg,
        process_error, thread_bg, thread_error, category.role, role_status,
        latency, latency_status, throughput, throughput_status);
}
