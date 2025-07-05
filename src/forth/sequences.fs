\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Arrays, stacks, queues and deques
\
\   The underlying functionality is a fixed block of memory, with four  associated properties:
\   1. A name
\   2. A length (number of available cells)
\   3. Pointer #1: used as stack pointer and deque and queue front pointer
\   4. Pointer #2: used as deque and queue back pointer.
\
\   Memory layoout:
\          <back pointer>
\          nfa	       - name field for the data structure
\          token field     - indicates the type of the object
\          size  	       - the number of cells allocated for data
\          pointer 1       - first pointer into the array
\          pointer 2       - second pointer into the array
\          <back pointer>
\
\        The token is required for interpretation, compilation, and for use by `see` and `dump`
\
\        Queues and Deques use the storage as a ring buffer with a maximum capacity equal to the number
\        of elements allocated.
\        The stack starts from index zero, with newer elements added above.
\
\        For all types, we use the name to reference the associated data
\        The count can be used to do your own bounds checking
\
\        For a general array:
\            Reference the name, and provide an index to access a specific element
\            Definition:     n array name       Creates a new array with n items accessed from 0 to n - 1
\            Read access:    n name a@          Returns the value at index n. Undefined if n < 0 or > n - 1
\            Write access: v n name a!          Stores the value v at index n.
\            Get length:       name ac@         Returns the number of items the array allocates.
\
\        For a stack:
\            Definition:     n stack name        Creates a new stack with a maximum depth of n
\            Read (pop):       name  s>          Returns the top value and adjusts the stack pointer
\            Write (push):   n name  >s          Adds a new value to the top of the stack and adjusts the stack pointer
\            Get (top):        name  s@          Returns the value on the top of the stack without modifying the stack.
\            Count:            name sc@          Returns the number of elements on the stack
\
\        For a deque:
\            Definition:     n deque name        Creates an empty deque of n elements.
\            Add to front:   n name >dq          Adds a new element to the front of the deque
\            Add to back:      name >>dq         Adds a new element to the back of the deque
\            Pop from front:   name dq>          Adds a new element to the back of the deque
\            Pop from back:    name dq>>         Adds a new element to the back of the deque
\            Count:            name dqc@         Returns the number of elements in the deque
\            
\        For a queue:
\            Definition:     n queue name        Creates an empty queue of n elements.
\            Add to back:    n name >q           Adds an element to the back of the queue
\            Pull from front:  name q>           Removes an element from the front of the queue
\            Count:            name qc@          Returns the number of elements in the queue

\ Create an array of n cells, with associated pointers and count.
\       Usage:    n array <name>
: array ( n -- )       
        create ARRAY ,          \ install the nfa and type token
        dup ,                   \ the number of elements
        0 , 0 ,                 \ the two pointers used for stacks, deques etc.
        allot (close)           \ allocate the required cells, and update pointers
;

\ Get the number of elements in an array
: ac@ ( addr -- n )
        @ ;               \ the use of the array name returns the address of the count

\ Return the value of cell n in an array
: a@ ( n addr -- v )
  + 2 +                   \ advance past the count and the two pointers
  @ ;                     \ this does no bounds checking!

\ Set cell n in an array
: a! ( v n addr -- )
  + 2 + ! ;

\ Create a stack of n cells. Stacks use pointer1 as the stack pointer
\    The stack pointer is an offset into the array, zero based
: stack ( n -- )
  array ;

\ Increment the stack pointer by the value on the stack
: (s+!) ( v addr -- )     \ addr is the address of the array (counter), not the stack pointer
  1 + +! ;

\ Get the stack pointer
: (sp@) ( addr -- v )     \ addr is the address of the array (counter), not the stack pointer
  1 + @ ;

\ Push an element onto the stack
: >s ( v addr -- )
  1 over (s+!)            \ increment the stack pointer
  (sp@) @ + ! ;           \ store the value into the address

\ Pop an element off the stack
: s> ( addr -- v )
  -1 over (s+!)           \ decrement the stack pointer
  (sp@) + 1 + @ ;         \ get the value previously pointed to by the stack pointer.
  
