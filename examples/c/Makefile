CC ?= cc
CPPFLAGS ?=
CFLAGS ?= -std=c11 -O2 -Wall -Wextra -Werror -Wpedantic

OPENDAL_MBT_LIB ?=
OPENDAL_MBT_NATIVE_LIBS ?=

CPPFLAGS += -I../../native/include

.PHONY: all syntax run clean

all: memory s3

memory: memory.c ../../native/include/opendal_mbt.h
	@if test -z "$(OPENDAL_MBT_LIB)"; then \
		echo "set OPENDAL_MBT_LIB to the built .a/.so/.dylib artifact" >&2; \
		exit 2; \
	fi
	$(CC) $(CPPFLAGS) $(CFLAGS) memory.c $(OPENDAL_MBT_LIB) \
		$(OPENDAL_MBT_NATIVE_LIBS) $(LDFLAGS) $(LDLIBS) -o $@

s3: s3.c ../../native/include/opendal_mbt.h
	@if test -z "$(OPENDAL_MBT_LIB)"; then \
		echo "set OPENDAL_MBT_LIB to the built .a/.so/.dylib artifact" >&2; \
		exit 2; \
	fi
	$(CC) $(CPPFLAGS) $(CFLAGS) s3.c $(OPENDAL_MBT_LIB) \
		$(OPENDAL_MBT_NATIVE_LIBS) $(LDFLAGS) $(LDLIBS) -o $@

syntax: memory.c ../../native/include/opendal_mbt.h
	$(CC) $(CPPFLAGS) $(CFLAGS) -fsyntax-only memory.c
	$(CC) $(CPPFLAGS) $(CFLAGS) -fsyntax-only s3.c

run: memory s3
	./memory
	./s3

clean:
	rm -f memory s3
