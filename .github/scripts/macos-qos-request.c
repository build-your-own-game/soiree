/* Controlled diagnostic injection only, never linked into a shipped host.
 * The default policy reader remains read-only in its separate library.
 * The debug-only pthread introspection hook also observes threads at START;
 * named mutating cases request QoS on each thread itself, before its callback.
 * https://github.com/apple-oss-distributions/libpthread/blob/main/include/pthread/introspection.h
 */
#include <errno.h>
#include <mach/mach.h>
#include <mach/task_policy.h>
#include <pthread/qos.h>
#include <pthread/introspection.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>

static pthread_introspection_hook_t previous_hook;
static qos_class_t worker_qos = QOS_CLASS_UNSPECIFIED;
static const char *worker_class;

static void report_policy(const char *phase) {
    uint64_t tid = 0;
    int tid_status = pthread_threadid_np(NULL, &tid);
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
        "\"thread_id\":%llu,\"thread_id_status\":%d,\"pthread_address\":\"%p\","
        "\"thread_requested_qos\":%u,\"process_background\":%d,"
        "\"process_background_errno\":%d,\"thread_background\":%d,"
        "\"thread_background_errno\":%d,\"task_role\":%d,\"task_role_status\":%d}\n",
        phase, getpid(), getppid(), getpgrp(), (unsigned long long)tid, tid_status,
        (void *)pthread_self(), qos_class_self(), process_bg,
        process_error, thread_bg, thread_error, category.role, role_status);
}

static void request_qos(qos_class_t qos, const char *name) {
    int result = pthread_set_qos_class_self_np(qos, 0);
    uint64_t tid = 0;
    int tid_status = pthread_threadid_np(NULL, &tid);
    fprintf(stderr,
        "SOIREE_MACOS_QOS_REQUEST {\"pid\":%d,\"thread_id\":%llu,\"thread_id_status\":%d,"
        "\"operation\":\"pthread_set_qos_class_self_np\","
        "\"requested_class\":\"%s\",\"requested_value\":%u,\"sdk_return\":%d}\n",
        getpid(), (unsigned long long)tid, tid_status, name, qos, result);
}

static void thread_hook(unsigned int event, pthread_t thread, void *address, size_t size) {
    if (previous_hook != NULL) previous_hook(event, thread, address, size);
    if (event == PTHREAD_INTROSPECTION_THREAD_CREATE) {
        // At CREATE only the target's address may be observed. Query the
        // current creator, never any property of the not-yet-started target.
        uint64_t creator = 0;
        int status = pthread_threadid_np(NULL, &creator);
        fprintf(stderr,
            "SOIREE_MACOS_THREAD_CREATE {\"pthread_address\":\"%p\","
            "\"creator_thread_id\":%llu,\"creator_thread_id_status\":%d}\n",
            (void *)thread, (unsigned long long)creator, status);
    }
    if (event == PTHREAD_INTROSPECTION_THREAD_START) {
        // START is documented to run in the created thread's own context.
        report_policy("thread-start-before-request");
        if (worker_qos != QOS_CLASS_UNSPECIFIED) {
            request_qos(worker_qos, worker_class);
            report_policy("thread-start-after-request");
        }
    }
}

__attribute__((constructor)) static void controlled_request(void) {
    const char *mode = getenv("SOIREE_MACOS_CONTROLLED_QOS");
    if (mode == NULL || (strcmp(mode, "user-initiated") != 0
            && strcmp(mode, "observe-threads") != 0
            && strcmp(mode, "utility") != 0
            && strcmp(mode, "utility-then-user-initiated") != 0)) {
        fputs("diagnostic QoS mutator requires an explicit controlled case\n", stderr);
        _exit(86);
    }
    if (strcmp(mode, "observe-threads") != 0) {
        worker_qos = strcmp(mode, "utility") == 0 ? QOS_CLASS_UTILITY : QOS_CLASS_USER_INITIATED;
        worker_class = strcmp(mode, "utility") == 0 ? "UTILITY" : "USER_INITIATED";
    }
    previous_hook = pthread_introspection_hook_install(thread_hook);
    report_policy("before-qos-request");
    if (strcmp(mode, "utility") == 0 || strcmp(mode, "utility-then-user-initiated") == 0) {
        request_qos(QOS_CLASS_UTILITY, "UTILITY");
        report_policy("after-utility-request");
    }
    if (strcmp(mode, "user-initiated") == 0 || strcmp(mode, "utility-then-user-initiated") == 0) {
        request_qos(QOS_CLASS_USER_INITIATED, "USER_INITIATED");
        report_policy("after-user-initiated-request");
    }
}
