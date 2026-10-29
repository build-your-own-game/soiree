/* Diagnostic only: compare one-shot native timers, on the same default-QoS worker. */
#include <dispatch/dispatch.h>
#include <errno.h>
#include <inttypes.h>
#include <mach/mach_time.h>
#include <pthread.h>
#include <pthread/qos.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/event.h>
#include <time.h>
#include <unistd.h>

static uint64_t now_ns(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) abort();
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static void *measure(void *argument) {
    const char *mode = argument;
    const int is_kqueue = strncmp(mode, "kqueue-", 7) == 0;
    const int critical = strcmp(mode, "kqueue-critical") == 0;
    const int fd = is_kqueue ? kqueue() : -1;
    if (is_kqueue && fd == -1) abort();
    dispatch_semaphore_t sem = dispatch_semaphore_create(0);
    if (!sem) abort();
    mach_timebase_info_data_t timebase;
    if (mach_timebase_info(&timebase) != KERN_SUCCESS) abort();
    uint64_t gaps[1024], late[1024];
    size_t count = 0;
    const uint64_t start = now_ns();
    uint64_t previous = start, done = 0;
    const qos_class_t qos = qos_class_self();
    do {
        const uint64_t due_ms = ((done + 1) * 1000 + 59) / 60;
        const uint64_t deadline = start + due_ms * 1000000;
        uint64_t current;
        while ((current = now_ns()) < deadline) {
            const uint64_t remaining = deadline - current;
            if (is_kqueue) {
                struct kevent change, event;
                EV_SET(&change, 1, EVFILT_TIMER, EV_ADD | EV_ONESHOT,
                       NOTE_NSECONDS | (critical ? NOTE_CRITICAL : 0), (intptr_t)remaining, NULL);
                int result = kevent(fd, &change, 1, &event, 1, NULL);
                if (result < 0 && errno == EINTR) continue;
                if (result != 1 || (event.flags & EV_ERROR) || event.filter != EVFILT_TIMER) abort();
            } else if (strcmp(mode, "mach-wait") == 0) {
                const uint64_t ticks = (uint64_t)((__uint128_t)remaining * timebase.denom / timebase.numer);
                kern_return_t result = mach_wait_until(mach_absolute_time() + ticks);
                if (result != KERN_SUCCESS && result != KERN_ABORTED) abort();
            } else if (strcmp(mode, "nanosleep") == 0) {
                struct timespec requested = {(time_t)(remaining / 1000000000), (long)(remaining % 1000000000)};
                if (nanosleep(&requested, NULL) != 0 && errno != EINTR) abort();
            } else if (strcmp(mode, "dispatch-semaphore") == 0) {
                if (dispatch_semaphore_wait(sem, dispatch_time(DISPATCH_TIME_NOW, (int64_t)remaining)) == 0) abort();
            } else { abort(); }
        }
        if (count >= 1024) abort();
        current = now_ns();
        gaps[count] = current - previous;
        late[count++] = current - deadline;
        previous = current;
        done = ((current - start) / 1000000) * 60 / 1000;
    } while (previous - start < 3000000000ULL);
    if (fd != -1 && close(fd) != 0) abort();
    dispatch_release(sem);
    printf("{\"mode\":\"%s\",\"qos\":%u,\"elapsed_ns\":%" PRIu64 ",\"max_spin_observation_gap_ns\":0,\"samples_gap_lateness_ns\":[", mode, qos, now_ns() - start);
    for (size_t i = 0; i < count; i++) {
        printf("%s[%" PRIu64 ",%" PRIu64 "]", i == 0 ? "" : ",", gaps[i], late[i]);
    }
    puts("]}");
    return NULL;
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    pthread_attr_t attr;
    if (pthread_attr_init(&attr) != 0) abort();
    if (pthread_attr_set_qos_class_np(&attr, QOS_CLASS_DEFAULT, 0) != 0) abort();
    pthread_t thread;
    if (pthread_create(&thread, &attr, measure, argv[1]) != 0) abort();
    if (pthread_attr_destroy(&attr) != 0) abort();
    if (pthread_join(thread, NULL) != 0) abort();
    return 0;
}
