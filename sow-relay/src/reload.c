/* Native adapter for F-Stack's shared reload protocol. No TCP or match state
 * is copied: a listener and every connection stay with their original worker. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include "ff_api.h"
#include "ff_config.h"
#include "ff_reload.h"

_Static_assert(DPDK_CONFIG_NUM == 32, "F-Stack 2.0 EAL argument ABI changed");
_Static_assert(sizeof(struct ff_zc_mbuf) == 24, "zero-copy buffer ABI changed");

#define SOW_RELOAD_MAGIC 0x534f5732u
#define SOW_RELOAD_VERSION 1u
#define SOW_MAX_WORKERS (FF_RELOAD_MAX_PROCS / FF_RELOAD_GEN_MAX)

struct sow_worker {
    uint32_t pid, ready, lobbies, connections;
    uint64_t pending, syncache, reported_ms;
};

struct sow_reload {
    uint32_t magic, version, size, workers, primary_ready, paused;
    uint64_t handover_ms;
    char foundation[65];
    struct ff_reload_state state;
    struct ff_reload_drain_state drain;
    struct sow_worker worker[FF_RELOAD_GEN_MAX][SOW_MAX_WORKERS];
    uint8_t port_owner[UINT16_MAX + 1u];
    pthread_mutex_t nonce_lock;
    struct { uint8_t digest[32]; uint64_t at; } nonce[4096];
};

static struct sow_reload *shared;
static int generation, queue = -1;
#define LOAD(p) __atomic_load_n((p), __ATOMIC_SEQ_CST)
#define STORE(p,v) __atomic_store_n((p), (v), __ATOMIC_SEQ_CST)

int sow_reload_open(int fd, uintptr_t address, unsigned workers,
                    const char *foundation, int gen, int qid)
{
    int create = fd < 0;
    struct stat st;
    if (workers == 0 || workers > SOW_MAX_WORKERS || gen < 0 || gen > 1
        || qid < -1 || qid >= (int)workers || strlen(foundation) != 64) {
        errno = EINVAL;
        return -1;
    }
    if (create) {
        fd = memfd_create("sow-relay-reload", MFD_CLOEXEC | MFD_ALLOW_SEALING);
        if (fd < 0) return -1;
        if (ftruncate(fd, sizeof(*shared)) != 0
            || fcntl(fd, F_ADD_SEALS, F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_SEAL) != 0) {
            close(fd);
            return -1;
        }
    }
    if (fstat(fd, &st) || (uint64_t)st.st_size != sizeof(*shared)) {
        if (create) close(fd);
        errno = EINVAL;
        return -1;
    }
    /* F-Stack publishes the drain extension's address in the shared block.
     * Preserve that address across exec; NEVER replace an existing mapping. */
    shared = mmap((void *)address, sizeof(*shared), PROT_READ | PROT_WRITE,
                  MAP_SHARED | (address ? MAP_FIXED_NOREPLACE : 0), fd, 0);
    if (shared == MAP_FAILED) {
        shared = NULL;
        if (create) close(fd);
        return -1;
    }
    if (create) {
        memset(shared, 0, sizeof(*shared));
        shared->magic = SOW_RELOAD_MAGIC;
        shared->version = SOW_RELOAD_VERSION;
        shared->size = sizeof(*shared);
        shared->workers = workers;
        memcpy(shared->foundation, foundation, 65);
        shared->state.magic = FF_RELOAD_STATE_MAGIC;
        shared->state.version = FF_RELOAD_STATE_VERSION;
        shared->state.len = sizeof(shared->state);
        shared->drain.magic = FF_RELOAD_DRAIN_MAGIC;
        shared->drain.len = sizeof(shared->drain);
        pthread_mutexattr_t attributes;
        int rc = pthread_mutexattr_init(&attributes);
        if (rc) { errno = rc; return -1; }
        rc = pthread_mutexattr_setpshared(&attributes, PTHREAD_PROCESS_SHARED);
        if (!rc) rc = pthread_mutexattr_setrobust(&attributes, PTHREAD_MUTEX_ROBUST);
        if (!rc) rc = pthread_mutex_init(&shared->nonce_lock, &attributes);
        pthread_mutexattr_destroy(&attributes);
        if (rc) { errno = rc; return -1; }
    }
    if (shared->magic != SOW_RELOAD_MAGIC
        || shared->version != SOW_RELOAD_VERSION
        || shared->size != sizeof(*shared) || shared->workers != workers
        || memcmp(shared->foundation, foundation, 65) != 0) {
        munmap(shared, sizeof(*shared));
        shared = NULL;
        if (create) close(fd);
        errno = EPROTO;
        return -1;
    }
    generation = gen;
    queue = qid;
    ff_reload_attach_state(&shared->state);
    if (ff_reload_drain_attach(&shared->drain, sizeof(shared->drain)) != 0) {
        errno = EPROTO;
        return -1;
    }
    ff_reload_set_gen(gen);
    ff_reload_set_slot(qid < 0 ? -1 : gen * SOW_MAX_WORKERS + qid);
    if (qid >= 0) STORE(&shared->worker[gen][qid].pid, (uint32_t)getpid());
    return fd;
}

uintptr_t sow_reload_address(void) { return (uintptr_t)shared; }
int sow_reload_managed(void) { return shared != NULL; }
int sow_reload_generation(void) { return generation; }
int sow_reload_active(void) {
    return shared ? (ff_reload_hw_locked() ? ff_reload_rx_owner_gen() : ff_reload_active_gen()) : 0;
}
int sow_reload_paused(void) { return shared ? LOAD(&shared->paused) != 0 : 0; }
void sow_reload_pause(int paused) { if (shared) STORE(&shared->paused, !!paused); }
int sow_reload_primary_ready(void) { return shared && LOAD(&shared->primary_ready); }
void sow_reload_primary_tick(void) { if (shared) STORE(&shared->primary_ready, 1); }

uint32_t sow_reload_pid(int gen, unsigned qid)
{
    return shared && gen >= 0 && gen < 2 && qid < shared->workers
        ? LOAD(&shared->worker[gen][qid].pid) : 0;
}

int sow_reload_ready(int gen, unsigned qid)
{
    return sow_reload_pid(gen, qid) && LOAD(&shared->worker[gen][qid].ready) == 2;
}

void sow_reload_worker_ready(void)
{
    if (shared && queue >= 0) {
        STORE(&shared->worker[generation][queue].ready, 1);
    }
}

void sow_reload_lobbies(uint32_t count)
{
    if (shared && queue >= 0) STORE(&shared->worker[generation][queue].lobbies, count);
}

void sow_reload_tick(void)
{
    static uint64_t next_report;
    if (shared && queue >= 0) {
        struct sow_worker *w = &shared->worker[generation][queue];
        struct timespec ts;
        if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) return;
        uint64_t now = (uint64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
        if (now >= next_report) {
            STORE(&w->connections, (uint32_t)ff_socket_drain_count());
            STORE(&w->pending, (uint64_t)ff_socket_snd_pending());
            STORE(&w->syncache, (uint64_t)ff_syncache_count());
            STORE(&w->reported_ms, now);
            next_report = now + 100;
        }
        if (LOAD(&w->ready) == 1) {
            ff_reload_publish_ready(generation * SOW_MAX_WORKERS + queue, getpid());
            STORE(&w->ready, 2);
        }
    }
}

int sow_reload_empty(int gen)
{
    if (!shared || gen < 0 || gen > 1) return 0;
    for (unsigned i = 0; i < shared->workers; ++i) {
        struct sow_worker *w = &shared->worker[gen][i];
        if (!LOAD(&w->pid)) continue; /* parent reaped this worker */
        if (!LOAD(&w->ready) || LOAD(&w->lobbies) || LOAD(&w->connections)
            || LOAD(&w->pending) || LOAD(&w->syncache)
            || LOAD(&w->reported_ms) <= LOAD(&shared->handover_ms)) return 0;
    }
    for (unsigned p = 0; p <= UINT16_MAX; ++p)
        if (LOAD(&shared->port_owner[p]) == gen + 1) return 0;
    return 1;
}

int sow_reload_owner(uint16_t port)
{
    return shared ? (int)LOAD(&shared->port_owner[port]) - 1 : generation;
}

int sow_reload_claim(uint16_t port)
{
    uint8_t expected = 0;
    if (!shared) return 0;
    if (__atomic_compare_exchange_n(&shared->port_owner[port], &expected,
            (uint8_t)(generation + 1), 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) return 1;
    return expected == generation + 1 ? 0 : -1;
}

void sow_reload_release(uint16_t port)
{
    uint8_t expected = generation + 1;
    if (shared) __atomic_compare_exchange_n(&shared->port_owner[port], &expected,
            0, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
}

int sow_reload_route(uint16_t port, uint16_t target)
{
    int owner = sow_reload_owner(port);
    return shared && owner >= 0 && owner != generation
        ? FF_DISPATCH_PEER_QUEUE(target) : target;
}

int sow_reload_begin(void)
{
    uint32_t epoch, target;
    if (!shared || ff_reload_hw_locked()) return -1;
    ff_reload_master_begin(&epoch, &target);
    memset(shared->worker[target], 0, sizeof(shared->worker[target]));
    ff_reload_drain_reset();
    return (int)target;
}

uint32_t sow_reload_park(void)
{
    uint32_t epoch;
    ff_reload_handover_arm(&epoch);
    ff_reload_rx_stopped_set(1);
    return epoch;
}

int sow_reload_parked(int gen, uint32_t epoch)
{
    for (unsigned i = 0; i < shared->workers; ++i)
        if (LOAD(&shared->worker[gen][i].pid)
            && !ff_reload_handover_parked(gen * SOW_MAX_WORKERS + i, epoch)) return 0;
    return 1;
}

void sow_reload_unpark(void) { ff_reload_rx_stopped_set(0); }
void sow_reload_abort(void) { ff_reload_master_abort(); ff_reload_rx_stopped_set(0); }
void sow_reload_commit(void) {
    if (ff_reload_rx_owner_gen() == ff_reload_target_gen()) ff_reload_master_complete();
    else ff_reload_master_abort();
    ff_reload_rx_stopped_set(0);
}
int sow_reload_handover(int target) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts)) return -1;
    STORE(&shared->handover_ms, (uint64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000);
    return ff_reload_rx_release(target);
}

void sow_reload_clear(int gen)
{
    if (shared && gen >= 0 && gen < 2)
        memset(shared->worker[gen], 0, sizeof(shared->worker[gen]));
}

void sow_reload_reaped(int gen, unsigned qid, uint32_t pid)
{
    if (shared && gen >= 0 && gen < 2 && qid < shared->workers
        && LOAD(&shared->worker[gen][qid].pid) == pid)
        STORE(&shared->worker[gen][qid].pid, 0);
}

/* Both generations may authenticate management traffic during handover.
 * Keep the existing one-use nonce rule across that process boundary. */
int sow_reload_nonce(const uint8_t digest[32], uint64_t now, uint64_t ttl)
{
    if (!shared) return -1;
    int rc = pthread_mutex_lock(&shared->nonce_lock), slot = -1, result = 0;
    if (rc == EOWNERDEAD) rc = pthread_mutex_consistent(&shared->nonce_lock);
    if (rc) return -1;
    for (unsigned i = 0; i < 4096; ++i) {
        uint64_t at = shared->nonce[i].at;
        if (!at || (now > at && now - at > ttl)) { if (slot < 0) slot = (int)i; continue; }
        if (!memcmp(shared->nonce[i].digest, digest, 32)) goto done;
    }
    if (slot < 0) { result = -1; goto done; }
    shared->nonce[slot].at = 0;
    memcpy(shared->nonce[slot].digest, digest, 32);
    shared->nonce[slot].at = now;
    result = 1;
done:
    if (pthread_mutex_unlock(&shared->nonce_lock)) return -1;
    return result;
}
