# Copyright © 2016 Bart Massey

CC = gcc
CFLAGS = -g -Wall

all: piperef ptyref

piperef: piperef.c
	$(CC) $(CFLAGS) -o piperef piperef.c

ptyref: ptyref.c
	$(CC) $(CFLAGS) -o ptyref ptyref.c

clean:
	-rm -f piperef ptyref
