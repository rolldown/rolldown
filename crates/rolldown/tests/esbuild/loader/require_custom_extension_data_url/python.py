# Define the byte sequence
byte_sequence = b'a\x00b\x80c\xFFd'

# Open a file in binary write mode
with open('test.custom', 'wb') as file:
    # Write the byte sequence to the file
    file.write(byte_sequence)
