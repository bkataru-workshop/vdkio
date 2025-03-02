# Exponential-Golomb coding - Wikipedia
From Wikipedia, the free encyclopedia

An **exponential-Golomb code** (or just **Exp-Golomb code**) is a type of [universal code](https://en.wikipedia.org/wiki/Universal_code_\(data_compression\) "Universal code (data compression)"). To encode any [nonnegative integer](https://en.wikipedia.org/wiki/Nonnegative_integer "Nonnegative integer") _x_ using the exp-Golomb code:

1.  Write down _x_+1 in binary
2.  Count the bits written, subtract one, and write that number of starting zero bits preceding the previous bit string.

The first few values of the code are:

```
 0 ⇒ 1 ⇒ 1
 1 ⇒ 10 ⇒ 010
 2 ⇒ 11 ⇒ 011
 3 ⇒ 100 ⇒ 00100
 4 ⇒ 101 ⇒ 00101
 5 ⇒ 110 ⇒ 00110
 6 ⇒ 111 ⇒ 00111
 7 ⇒ 1000 ⇒ 0001000
 8 ⇒ 1001 ⇒ 0001001
...[1]

```


In the above examples, consider the case 3. For 3, x+1 = 3 + 1 = 4. 4 in binary is '100'. '100' has 3 bits, and 3-1 = 2. Hence add 2 zeros before '100', which is '00100'

Similarly, consider 8. '8 + 1' in binary is '1001'. '1001' has 4 bits, and 4-1 is 3. Hence add 3 zeros before 1001, which is '0001001'.

This is identical to the [Elias gamma code](https://en.wikipedia.org/wiki/Elias_gamma_code "Elias gamma code") of _x_+1, allowing it to encode 0.[\[2\]](#cite_note-2)

Extension to negative numbers
-----------------------------



Exp-Golomb coding is used in the [H.264/MPEG-4 AVC](https://en.wikipedia.org/wiki/H.264/MPEG-4_AVC "H.264/MPEG-4 AVC") and H.265 [High Efficiency Video Coding](https://en.wikipedia.org/wiki/High_Efficiency_Video_Coding "High Efficiency Video Coding") video compression standards, in which there is also a variation for the coding of signed numbers by assigning the value 0 to the binary codeword '0' and assigning subsequent codewords to input values of increasing magnitude (and alternating sign, if the field can contain a negative number):

```
 0 ⇒ 0 ⇒ 1 ⇒ 1
 1 ⇒ 1 ⇒ 10 ⇒ 010
−1 ⇒ 2 ⇒ 11 ⇒ 011
 2 ⇒ 3 ⇒ 100 ⇒ 00100
−2 ⇒ 4 ⇒ 101 ⇒ 00101
 3 ⇒ 5 ⇒ 110 ⇒ 00110
−3 ⇒ 6 ⇒ 111 ⇒ 00111
 4 ⇒ 7 ⇒ 1000 ⇒ 0001000
−4 ⇒ 8 ⇒ 1001 ⇒ 0001001
...[1]

```


In other words, a non-positive integer _x_≤0 is mapped to an even integer −2_x_, while a positive integer _x_\>0 is mapped to an odd integer 2_x_−1.

Exp-Golomb coding is also used in the [Dirac video codec](https://en.wikipedia.org/wiki/Dirac_\(video_compression_format\) "Dirac (video compression format)").[\[3\]](#cite_note-3)

Generalization to order _k_
---------------------------



To encode larger numbers in fewer bits (at the expense of using more bits to encode smaller numbers), this can be generalized using a [nonnegative integer](https://en.wikipedia.org/wiki/Nonnegative_integer "Nonnegative integer") parameter  _k_. To encode a nonnegative integer _x_ in an order-_k_ exp-Golomb code:

1.  Encode ⌊_x_/2_k_⌋ using order-0 exp-Golomb code described above, then
2.  Encode _x_ mod 2_k_ in binary with k bits

An equivalent way of expressing this is:

1.  Encode _x_+2_k_−1 using the order-0 exp-Golomb code (i.e. encode _x_+2_k_ using the Elias gamma code), then
2.  Delete _k_ leading zero bits from the encoding result


Exp-Golomb-k coding examples


*  x : 0
  * k=0: 1
  * k=1: 10
  * k=2: 100
  * k=3: 1000
  * 10
  *  x : 0001011
  * k=0: 001100
  * k=1: 01110
  * k=2: 010010
  * k=3: 20
  * 000010101
  *  x : 00010110
  * k=0: 0011000
  * k=1: 011100
  * k=2: 
  * k=3: 
*  x : 1
  * k=0: 010
  * k=1: 11
  * k=2: 101
  * k=3: 1001
  * 11
  *  x : 0001100
  * k=0: 001101
  * k=1: 01111
  * k=2: 010011
  * k=3: 21
  * 000010110
  *  x : 00010111
  * k=0: 0011001
  * k=1: 011101
  * k=2: 
  * k=3: 
*  x : 2
  * k=0: 011
  * k=1: 0100
  * k=2: 110
  * k=3: 1010
  * 12
  *  x : 0001101
  * k=0: 001110
  * k=1: 0010000
  * k=2: 010100
  * k=3: 22
  * 000010111
  *  x : 00011000
  * k=0: 0011010
  * k=1: 011110
  * k=2: 
  * k=3: 
*  x : 3
  * k=0: 00100
  * k=1: 0101
  * k=2: 111
  * k=3: 1011
  * 13
  *  x : 0001110
  * k=0: 001111
  * k=1: 0010001
  * k=2: 010101
  * k=3: 23
  * 000011000
  *  x : 00011001
  * k=0: 0011011
  * k=1: 011111
  * k=2: 
  * k=3: 
*  x : 4
  * k=0: 00101
  * k=1: 0110
  * k=2: 01000
  * k=3: 1100
  * 14
  *  x : 0001111
  * k=0: 00010000
  * k=1: 0010010
  * k=2: 010110
  * k=3: 24
  * 000011001
  *  x : 00011010
  * k=0: 0011100
  * k=1: 00100000
  * k=2: 
  * k=3: 
*  x : 5
  * k=0: 00110
  * k=1: 0111
  * k=2: 01001
  * k=3: 1101
  * 15
  *  x : 000010000
  * k=0: 00010001
  * k=1: 0010011
  * k=2: 010111
  * k=3: 25
  * 000011010
  *  x : 00011011
  * k=0: 0011101
  * k=1: 00100001
  * k=2: 
  * k=3: 
*  x : 6
  * k=0: 00111
  * k=1: 001000
  * k=2: 01010
  * k=3: 1110
  * 16
  *  x : 000010001
  * k=0: 00010010
  * k=1: 0010100
  * k=2: 011000
  * k=3: 26
  * 000011011
  *  x : 00011100
  * k=0: 0011110
  * k=1: 00100010
  * k=2: 
  * k=3: 
*  x : 7
  * k=0: 0001000
  * k=1: 001001
  * k=2: 01011
  * k=3: 1111
  * 17
  *  x : 000010010
  * k=0: 00010011
  * k=1: 0010101
  * k=2: 011001
  * k=3: 27
  * 000011100
  *  x : 00011101
  * k=0: 0011111
  * k=1: 00100011
  * k=2: 
  * k=3: 
*  x : 8
  * k=0: 0001001
  * k=1: 001010
  * k=2: 01100
  * k=3: 010000
  * 18
  *  x : 000010011
  * k=0: 00010100
  * k=1: 0010110
  * k=2: 011010
  * k=3: 28
  * 000011101
  *  x : 00011110
  * k=0: 000100000
  * k=1: 00100100
  * k=2: 
  * k=3: 
*  x : 9
  * k=0: 0001010
  * k=1: 001011
  * k=2: 01101
  * k=3: 010001
  * 19
  *  x : 000010100
  * k=0: 00010101
  * k=1: 0010111
  * k=2: 011011
  * k=3: 29
  * 000011110
  *  x : 00011111
  * k=0: 000100001
  * k=1: 00100101
  * k=2: 
  * k=3: 


*   [Elias gamma (γ) coding](https://en.wikipedia.org/wiki/Elias_gamma_coding "Elias gamma coding")
*   [Elias delta (δ) coding](https://en.wikipedia.org/wiki/Elias_delta_coding "Elias delta coding")
*   [Elias omega (ω) coding](https://en.wikipedia.org/wiki/Elias_omega_coding "Elias omega coding")
*   [Universal code](https://en.wikipedia.org/wiki/Universal_code_\(data_compression\) "Universal code (data compression)")

1.  ^   Richardson, Iain (2010). [_The H.264 Advanced Video Compression Standard_](https://books.google.com/books?id=LJoDiPnBzQ8C&q=Exponential-Golomb+coding&pg=PA221). Wiley. pp. 208, 221. [ISBN](https://en.wikipedia.org/wiki/ISBN_\(identifier\) "ISBN (identifier)") [978-0-470-51692-8](https://en.wikipedia.org/wiki/Special:BookSources/978-0-470-51692-8 "Special:BookSources/978-0-470-51692-8").
2.   Rupp, Markus (2009). [_Video and Multimedia Transmissions over Cellular Networks: Analysis, Modelling and Optimization in Live 3G Mobile Networks_](https://books.google.com/books?id=H9hUBT-JvUoC&q=Exponential-Golomb+coding&pg=PA149). Wiley. p. 149. [ISBN](https://en.wikipedia.org/wiki/ISBN_\(identifier\) "ISBN (identifier)") [9780470747766](https://en.wikipedia.org/wiki/Special:BookSources/9780470747766 "Special:BookSources/9780470747766").
3.   ["Dirac Specification"](https://web.archive.org/web/20150503015104/http://diracvideo.org/download/specification/dirac-spec-latest.pdf) (PDF). BBC. Archived from the original on 2015-05-03. Retrieved 9 March 2011.