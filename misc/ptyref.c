#define _GNU_SOURCE
#define _XOPEN_SOURCE
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

static char strbuf[1024];

int main(void) {
    int ptmx = getpt();
    assert(ptmx >= 0);
    assert(grantpt(ptmx) == 0);
    assert(unlockpt(ptmx) == 0);
    pid_t pid = fork();
    assert(pid >= 0);
    switch (pid) {
    case 0: {
            int spid = setsid();
            if (spid == -1) {
                perror("setsid");
                abort();
            }
            char *pts = ptsname(ptmx);
            assert(pts);
            int pty = open(pts, O_RDWR, 0);
            assert (pty >= 0);
            int tty = open("/dev/tty", O_RDWR, 0);
            FILE *ttyf = fdopen(tty, "r+");
            assert (ttyf);
            fprintf(ttyf, "hello world\n");
            fflush(ttyf);
            break;
        }
    default: {
            FILE *ptmxf = fdopen(ptmx, "r+");
            fgets(strbuf, sizeof(strbuf), ptmxf);
            fprintf(stderr, "got %s", strbuf);
            break;
        }
    }
    return 0;
}
