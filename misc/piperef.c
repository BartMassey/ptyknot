/* Copyright © 2016 Bart Massey */

/* Demonstrate pipes and redirecting child stdout. */

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <errno.h>
#include <string.h>

static char strbuf[1024];

int main(void) {
    int pipefds[2];
    assert(pipe(pipefds) == 0);
    pid_t pid = fork();
    assert(pid >= 0);
    if (pid == 0) {
        assert(close(pipefds[0]) == 0);
        int spid = setsid();
        if (spid == -1) {
            perror("setsid");
            abort();
        }
        assert(dup2(pipefds[1], 1) == 1);
        printf("hello world\n");
        exit(0);
    }
    assert(close(pipefds[1]) == 0);
    FILE *child_stdout = fdopen(pipefds[0], "r");
    assert(child_stdout);
    fgets(strbuf, sizeof(strbuf), child_stdout);
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
