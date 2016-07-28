/* Copyright © 2016 Bart Massey */

/* Demonstrate reading from child controlling terminal. */

#define _XOPEN_SOURCE
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <errno.h>
#include <string.h>
#include <fcntl.h>

static char strbuf[1024];

int main(void) {
    int ptmx = open("/dev/ptmx", O_RDONLY);
    assert(ptmx >= 0);
    assert(grantpt(ptmx) == 0);
    assert(unlockpt(ptmx) == 0);
    pid_t pid = fork();
    assert(pid >= 0);
    if (pid == 0) {
        int spid = setsid();
        if (spid == -1) {
            perror("setsid");
            abort();
        }
        char *pts = ptsname(ptmx);
        assert(pts);
        int pty = open(pts, O_RDWR, 0);
        assert(pty >= 0);
        assert(close(pty) == 0);
        int tty = open("/dev/tty", O_RDWR, 0);
        FILE *ttyf = fdopen(tty, "r+");
        assert (ttyf);
        fprintf(ttyf, "hello world\n");
        exit(0);
    }
    FILE *ptmxf = fdopen(ptmx, "r");
    fgets(strbuf, sizeof(strbuf), ptmxf);
    fprintf(stderr, "got %s", strbuf);
    int wstatus = 0;
    pid_t result = waitpid(pid, &wstatus, 0);
    if (result == -1) {
        perror("waitpid");
        exit(1);
    }
    if (WEXITSTATUS(wstatus) != 0) {
        fprintf(stderr, "child exited with status %d\n", WEXITSTATUS(wstatus));
        exit(1);
    }
    return 0;
}
