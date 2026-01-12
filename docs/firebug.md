Comprehensive NES Mapper Document v0.80 by \Firebug\ (emulation@biosys.net)
Best viewed under DOS EDIT
Information provided by FanWen, Y0SHi, D, and Jim Geffre
Free for non-commercial use
This document is dedicated to Vertigo 2099, the greatest ROM releasing
group on the net, of which I am a proud member. And Jim Geffre, the author
of PCNES and GBE. Thanks for all the help.

****************************************************************************
Why did I write this? There just isn't any other good and widely available
documentation on the "extended" mappers (16 and above). FanWen's documents
(on which the extended mapper section is largely based) are an excellent
source of information, but I discovered several errors in them, and more
importantly, they are quite hard to obtain. Goroh's documents, released
more recently, are easier to get and fairly accurate, but the Japanese and
broken English text can make reading through them an ordeal.
I also wanted to combine the several documents I had on MMC1 into one
comprehensive source. Any mapper information not given here is appreciated.
I cannot guarantee that all this information is 100% accurate. What I can
guarantee is that I have put in the maximum possible amount of effort to
get this document as accurate as possible. If you notice a mistake,
please tell me about it, and I'll give you credit for the fix.
I hope it doesn't take me a year to update it again...
****************************************************************************

          Ŀ
   History  
            

0.80            Numerous small mistakes have been fixed. A comment was added
                regarding mapper #21 and the ambiguities in the .NES format.
                Mapper numbers are listed for the exotic mappers, although
                these may change with the standardization of the format.
                Many new mapper descriptions added.
0.70            First public release
0.68            Sunsoft mapper 4 (AfterBurner ][) added
0.67            Combined mapper #21 with the other VRC4 type
0.65            Added sections on Sunsoft FME-7 and VRC4 type B mappers
0.61            Corrected serious errors in mapper #21 section
                Added some preliminary MMC5 information
0.60            Initial release

****************************************************************************

                 Ŀ
   Mapper 1: MMC1  
                   

                                                                     Ŀ
   This mapper is used on numerous U.S. and Japanese games, including  
   Legend of Zelda, Metroid, Rad Racer, MegaMan 2, and many others.    
                                                                       

                Ŀ                                                         Ŀ
   $8000   $9FFF  Ĵ RxxCFHPM                                                
   (Register 0)                                                              
                                 Mirroring Flag                              
                                  0 = Horizontal                             
                                  1 = Vertical                               
                                                                             
                                 One Screen Mirroring                        
                                  0 = All pages mirrored from PPU $2000      
                                  1 = Regular mirroring                      
                                                                             
                                 PRG Switching Area                          
                                  0 = Swap ROM bank at $C000                 
                                  1 = Swap ROM bank at $8000                 
                                                                             
                                 PRG Switching Size                          
                                  0 = Swap 32K of ROM at $8000               
                                  1 = Swap 16K of ROM based on bit 2         
                                                                             
                                 <Carts with VROM>                           
                                 VROM Switching Size                         
                                  0 = Swap 8K of VROM at PPU $0000           
                                  1 = Swap 4K of VROM at PPU $0000 and $1000 
                                 <1024K carts>                               
                                  0 = Ignore 256K selection register 0       
                                  1 = Acknowledge 256K selection register 1  
                                                                             
                                 Reset Port                                  
                                  0 = Do nothing                             
                                  1 = Reset register 0                       
                                                                             

                Ŀ                                                         Ŀ
   $A000   $BFFF  Ĵ RxxPCCCC                                                
   (Register 1)                                                              
                                  Select VROM bank at $0000                  
                                  If bit 4 of register 0 is off, then switch 
                                  a full 8K bank. Otherwise, switch 4K only. 
                                                                             
                                  256K ROM Selection Register 0              
                                  <512K carts>                               
                                  0 = Swap banks from first 256K of PRG      
                                  1 = Swap banks from second 256K of PRG     
                                  <1024K carts with bit 4 of register 0 off> 
                                  0 = Swap banks from first 256K of PRG      
                                  1 = Swap banks from third 256K of PRG      
                                  <1024K carts with bit 4 of register 0 on>  
                                  Low bit of 256K PRG bank selection         
                                                                             
                                  Reset Port                                 
                                  0 = Do nothing                             
                                  1 = Reset register 1                       
                                                                             

                Ŀ                                                         Ŀ
   $C000   $DFFF  Ĵ RxxPCCCC                                                
   (Register 2)                                                              
                                Select VROM bank at $1000                    
                                 If bit 4 of register 0 is on, then switch   
                                 a 4K bank at $1000. Otherwise ignore it.    
                                                                             
                                256K ROM Selection Register 1                
                                 <1024K carts with bit 4 of register 0 off>  
                                  Store but ignore this bit (base 256K       
                                  selection on 256K selection register 0)    
                                 <1024K carts with bit 4 of register 0 on>   
                                  High bit of 256K PRG bank selection        
                                                                             
                                Reset Port                                   
                                 0 = Do nothing                              
                                 1 = Reset register 2                        
                                                                             

                Ŀ                                                         Ŀ
   $E000   $FFFF  Ĵ RxxxCCCC                                                
   (Register 3)                                                              
                                 Select ROM bank                             
                                 Size is determined by bit 3 of register 0   
                                 If it's a 32K bank, it will be swapped at   
                                 $8000. (NOTE: In this case, the value       
                                 written should be shifted right 1 bit to    
                                 get the actual value.) If it's a 16K bank,  
                                 it will be selected at $8000 or $C000 based 
                                 on the value in bit 2 of register 0.        
                                 Don't forget to also account for the 256K   
                                 block swapping if the PRG size is 512K or   
                                 more.                                       
                                                                             
                                 Reset Port                                  
                                 0 = Do nothing                              
                                 1 = Reset register 3                        
                                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K bank into $C000. Normally,
           the first 16K bank is swapped via register 3 and the last bank
           remains "hard-wired". However, bit 2 of register 0 can change
           this. If it's clear, then the first 16K bank is "hard-wired" to
           bank zero, and the last bank is swapped via register 3. Bit 3
           of register 0 will override either of these states, and allow
           the whole 32K to be swapped.
        - MMC1 ports are only one bit. Therefore, a value will be written
           into these registers one bit at a time. Values aren't used until
           the entire 5-bit array is filled. This buffering can be reset
           by writing bit 7 of the register. Note that MMC1 only has one
           5-bit array for this data, not a separate one for each register.

****************************************************************************

                  Ŀ
   Mapper 2: UNROM  
                    

                                                                     Ŀ
   This mapper is used on many older U.S. and Japanese games, such as  
   Castlevania, MegaMan, Ghosts & Goblins, and Amagon.                 
                                                                       

                Ŀ                                                   Ŀ
   $8000   $FFFF          Ĵ PPPPPPPP                                  
                                                                       
                                                                       
                                                                       
                                         Select 16K ROM bank at $8000  
                                                                       

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. This last 16K bank is permanently "hard-wired" to $C000,
           and it cannot be swapped.
        - This mapper has no provisions for VROM; therefore, all carts
           using it have 8K of VRAM at PPU $0000.
        - Most carts with this mapper are 128K. A few, mostly Japanese
           carts, such as Final Fantasy 2 and Dragon Quest 3, are 256K.
        - Overall, this is one of the easiest mappers to implement in
           a NES emulator.

****************************************************************************

                  Ŀ
   Mapper 3: CNROM  
                    

                                                                     Ŀ
   This mapper is used on many older U.S. and Japanese games, such as  
   Solomon's Key, Gradius, and Hudson's Adventure Island.              
                                                                       

                Ŀ                                                 Ŀ
   $8000   $FFFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 8K VROM bank at PPU $0000  
                                                                     

 Notes: - The ROM size is either 16K or 32K and is not switchable. It is
           loaded in the same manner as a NROM game; in other words,
           it's loaded at $8000 if it's a 32K ROM size, and at $C000 if
           it's a 16K ROM size. (This is because a 6502 CPU requires
           several vectors to be at $FFFA   $FFFF, and therefore ROM needs
           to be there at all times.)
        - The first 8K VROM bank is swapped into PPU $0000 when the cart
           is reset.
        - This is probably the simplest memory mapper and can easily be
           incorporated into a NES emulator.

****************************************************************************

                 Ŀ
   Mapper 4: MMC3  
                   

                                                                     Ŀ
   A great majority of newer NES games (early 90's) use this mapper,   
   both U.S. and Japanese. Among the better-known MMC3 titles are      
   Super Mario Bros. 2 and 3, MegaMan 3, 4, 5, and 6, and Crystalis.   
                                                                       

        Ŀ                                                         Ŀ
   $8000    Ĵ CPxxxNNN                                              
                                                                     
                          Command Number                             
                           0 - Select 2 1K VROM pages at PPU $0000   
                           1 - Select 2 1K VROM pages at PPU $0800   
                           2 - Select 1K VROM page at PPU $1000      
                           3 - Select 1K VROM page at PPU $1400      
                           4 - Select 1K VROM page at PPU $1800      
                           5 - Select 1K VROM page at PPU $1C00      
                           6 - Select first switchable ROM page      
                           7 - Select second switchable ROM page     
                                                                     
                          PRG Address Select                         
                           0 - Enable swapping for $8000 and $A000   
                           1 - Enable swapping for $A000 and $C000   
                                                                     
                          CHR Address Select                         
                           0 - Use normal address for commands 0-5   
                           1 - XOR command 0-5 address with $1000    
                                                                     

        Ŀ                                                 Ŀ
   $8001    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Page Number for Command           
                            Activates the command number     
                            written to bits 0-2 of $8000     
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ xxxxxxxM                                      
                                                             
                                                             
                                                             
                           Mirroring Select                  
                            0 - Horizontal mirroring         
                            1 - Vertical mirroring           
               NOTE: I don't have any confidence in the      
                     accuracy of this information.           
                                                             

        Ŀ                                                 Ŀ
   $A001    Ĵ Sxxxxxxx                                      
                                                             
                                                             
                                                             
                           SaveRAM Toggle                    
                            0 - Disable $6000-$7FFF          
                            1 - Enable $6000-$7FFF           
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           IRQ Counter Register              
                            The IRQ countdown value is       
                            stored here.                     
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           IRQ Latch Register                
                            A temporary value is stored      
                            here.                            
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            Any value written here will      
                            disable IRQ's and copy the       
                            latch register to the actual     
                            IRQ counter register.            
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 1            
                            Any value written here will      
                            enable IRQ's.                    
                                                             

 Notes: - Two of the 8K ROM banks in the PRG area are switchable.
           The other two are "hard-wired" to the last two banks in
           the cart. The default setting is switchable banks at
           $8000 and $A000, with banks 0 and 1 being swapped in
           at reset. Through bit 6 of $8000, the hard-wiring can
           be made to affect $8000 and $E000 instead of $C000 and
           $E000. The switchable banks, whatever their addresses,
           can be swapped through commands 6 and 7.
        - A cart will first write the command and base select number
           to $8000, then the value to be used to $8001.
        - On carts with VROM, the first 8K of VROM is swapped into
           PPU $0000 on reset. On carts without VROM, as always, there
           is 8K of VRAM at PPU $0000.

****************************************************************************

                 Ŀ
   Mapper 5: MMC5  
                   

                                                                     Ŀ
   This mapper appears in a few newer NES titles, most notably         
   Castlevania 3. Some other games such as Uncharted Waters and        
   several Koei titles also use this mapper. Thanks to D and           
   Jim Geffre for this information.                                    
                                                                       

        Ŀ                                               Ŀ
   $5103    Ĵ xxxxxxSS                                    
                                                           
                                                           
                                                           
                         Sprite CHR bank size              
                          0 - One 8K bank                  
                          1 - Two 4K banks                 
                          2 - Three 2K banks               
                          3 - Four 1K banks                
                                                           

        Ŀ                                                  Ŀ
   $5104    Ĵ xxxxxxCT                                       
                                                              
                                                              
                                                              
                           EXRAM background tile select       
                            0 - Normal tile support           
                            1 - Enable EXRAM for tiles        
                                                              
                           EXRAM color select                 
                            0 - EXRAM color off               
                            1 - Enable EXRAM color expansion  
                                                              

        Ŀ                                               Ŀ
   $5105    Ĵ MMMMMMMM                                    
                                                           
                                                           
                                                           
                         $2000 nametable select            
                          Select nametable for $2000       
                                                           
                         $2400 nametable select            
                          Select nametable for $2400       
                                                           
                         $2800 nametable select            
                          Select nametable for $2800       
                                                           
                         $2C00 nametable select            
                           Select nametable for $2C00      
                                                           

        Ŀ                                                 Ŀ
   $5114    Ĵ UPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                                                             
                           PRG Bank Activation               
                            0 = Bank contains all $FFs       
                            1 = Bank contains 8K of ROM      
                                 selected from bits 0-7      
                                                             

        Ŀ                                                 Ŀ
   $5115    Ĵ UPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             
                           PRG Bank Activation               
                            0 = Bank contains all $FFs       
                            1 = Bank contains 8K of ROM      
                                 selected from bits 0-7      
                                                             

        Ŀ                                                 Ŀ
   $5116    Ĵ UPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $C000       
                                                             
                           PRG Bank Activation               
                            0 = Bank contains all $FFs       
                            1 = Bank contains 8K of ROM      
                                 selected from bits 0-7      
                                                             

        Ŀ                                                 Ŀ
   $5120    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            Only active if 1K switching is   
                            active via $5103                 
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5121    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           (If 1K switching is active        
                            via $5103)                       
                           Select 1K VROM bank at PPU $0400  
                           (If 2K switching is active        
                            via $5103)                       
                           Select 2K VROM bank at PPU $0000  
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5122    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            Only active if 1K switching is   
                            active via $5103                 
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5123    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           (If 1K switching is active        
                            via $5103)                       
                           Select 1K VROM bank at PPU $0C00  
                           (If 2K switching is active        
                            via $5103)                       
                           Select 2K VROM bank at PPU $0800  
                           (If 4K switching is active        
                            via $5103)                       
                           Select 4K VROM bank at PPU $0000  
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5124    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            Only active if 1K switching is   
                            active via $5103                 
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5125    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           (If 1K switching is active        
                            via $5103)                       
                           Select 1K VROM bank at PPU $1400  
                           (If 2K switching is active        
                            via $5103)                       
                           Select 2K VROM bank at PPU $1000  
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5126    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            Only active if 1K switching is   
                            active via $5103                 
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5127    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           (If 1K switching is active        
                            via $5103)                       
                           Select 1K VROM bank at PPU $1C00  
                           (If 2K switching is active        
                            via $5103)                       
                           Select 2K VROM bank at PPU $1800  
                           (If 4K switching is active        
                            via $5103)                       
                           Select 4K VROM bank at PPU $1000  
                           (If 8K switching is active        
                            via $5103)                       
                           Select 8K VROM bank at PPU $0000  
                            This CHR selection is used for   
                            drawing sprites only.            
                                                             

        Ŀ                                                 Ŀ
   $5128    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0000  
                            This CHR selection is used only  
                            for drawing the nametables if    
                            EXRAM is not activated.          
                                                             

        Ŀ                                                 Ŀ
   $5129    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0800  
                            This CHR selection is used only  
                            for drawing the nametables if    
                            EXRAM is not activated.          
                                                             

        Ŀ                                                 Ŀ
   $512A    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1000  
                            This CHR selection is used only  
                            for drawing the nametables if    
                            EXRAM is not activated.          
                                                             

        Ŀ                                                 Ŀ
   $512B    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1800  
                            This CHR selection is used only  
                            for drawing the nametables if    
                            EXRAM is not activated.          
                                                             

 Notes: - Much of this information is incomplete and possibly inaccurate.
        - To learn about MMC5's EXRAM system, read Y0SHi's NESTECH
           document. Note that Castlevania 3 doesn't use EXRAM but
           the Koei games (Bandit Kings of Ancient China, Gemfire, etc.)
           do use it.
         - On reset, all ROM banks are set to the LAST 8K bank in the
            cartridge. The last 8K of this is "hard-wired" and cannot
            be swapped. (As far as I know.)
         - MMC5 has its own sound chip, which is only used in Japanese
            games. I do not know how it works.

****************************************************************************

                      Ŀ
   Mapper 6: FFE F4xxx  
                        

                                                                     Ŀ
   Several hacked Japanese titles use this mapper, such as the hacked  
   version of Wai Wai World. The unhacked versions of these games      
   seem to use a Konami VRC mapper, and it's better to use them if     
   possible.                                                           
                                                                       

        Ŀ                                                     Ŀ
   $42FC    Ĵ xxxPxxxx                                          
                                                                 
                                                                 
                                                                 
                             Unknown                             
                                                                 

        Ŀ                                                     Ŀ
   $42FD    Ĵ xxxMxxxx                                          
                                                                 
                                                                 
                                                                 
                             Unknown                             
                                                                 

        Ŀ                                                     Ŀ
   $42FE    Ĵ xxxPxxxx                                          
                                                                 
                                                                 
                                                                 
                             Page Select                         
                              0 - Mirror pages from PPU $2400    
                              1 - Mirror pages from PPU $2000    
                                                                 

        Ŀ                                                     Ŀ
   $42FF    Ĵ xxxMxxxx                                          
                                                                 
                                                                 
                                                                 
                             Mirroring Select                    
                              0 - Horizontal mirroring           
                              1 - Vertical mirroring             
                                                                 

        Ŀ                                                  Ŀ
   $43FE    Ĵ CCCCCCPP                                       
                                                              
                   Ĵ       512K PRG Select                   
                                                              
                            512K CHR Select                   
               NOTE: I don't have any confidence in the       
                     accuracy of this information.            
                                                              

        Ŀ                                                  Ŀ
   $4500    Ĵ DESSWPPP                                       
                                                              
                            PPU Mode Select                   
                             1 - 32K                          
                             5 - 256K plus EXRAM              
                             7 - 256K                         
                                                              
                            SW Pin                            
                             I have no idea what this does.   
                                                              
                            SaveRAM Toggle                    
                             0 - No SaveRAM                   
                             1 - SaveRAM                      
                                                              
                            Execution Mode                    
                             0 - Do nothing                   
                             1 - Execute game                 
                                                              
                            Medium                            
                             0 - Famicom Disk System          
                             1 - Cartridge                    
                                                              

        Ŀ                                                 Ŀ
   $4501    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            Any value written here will      
                            disable IRQ's.                   
                                                             

        Ŀ                                                 Ŀ
   $4502    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           Low byte of IRQ counter           
                                                             

        Ŀ                                                 Ŀ
   $4503    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           High byte of IRQ counter and      
                           IRQ Control Register 1            
                            Any value written here will      
                            enable IRQ's.                    
                                                             

                Ŀ                                                   Ŀ
   $8000   $FFFF     Ĵ xxPPPPCC                                       
                                                                       
                            Ĵ       Pattern Table Select              
                                                                       
                                      Select 16K ROM bank at $8000     
                                                                       

 Notes: - The IRQ counter is incremented at each scanline. When it reaches
           $FFFF, it is reset to zero and an IRQ interrupt is executed.
        - I am not sure if all my information about this mapper is accurate.

****************************************************************************

                  Ŀ
   Mapper 7: AOROM  
                    

                                                                     Ŀ
   Numerous games released by Rare Ltd. use this mapper, such as       
   Battletoads, Wizards & Warriors, and Solar Jetman.                  
                                                                       

                Ŀ                                                 Ŀ
   $8000   $FFFF    Ĵ xxxSPPPP                                      
                                                                     
                             Ĵ                                      
                                                                     
                                    Select 32K ROM bank at $8000     
                                                                     
                                   One Screen Mirroring              
                                    0 = Mirror pages from PPU $2000  
                                    1 = Mirror pages from PPU $2400  
                                                                     

 Notes: - The first 32K ROM bank is swapped into $8000 when the cart is
           started or reset.
        - This mapper has no provisions for VROM; therefore, all carts
           using it have 8K of VRAM at PPU $0000.
        - Many carts using this mapper need precise NES timing to work
           properly. If you're writing an emulator, be sure that you have
           provisions for switching screens during refresh, and be sure the
           one screen mirroring is emulated properly. Also make sure that
           you have provisions for palette changes in midframe and for
           special handling of mid-HBlank writes to $2006.

****************************************************************************

                      Ŀ
   Mapper 8: FFE F3xxx  
                        

                                                                     Ŀ
   Several hacked Japanese titles use this mapper, such as the hacked  
   version of Doraemon.                                                
                                                                       

                Ŀ                                                   Ŀ
   $8000   $FFFF     Ĵ PPPPPCCC                                       
                                                                       
                           Ĵ        Select 8K VROM bank at PPU $0000  
                                                                       
                                     Select 16K ROM bank at $8000      
                                                                       

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the SECOND 16K ROM bank is loaded into
           $C000. This 16K bank is permanently "hard-wired" to $C000, and it
           cannot be swapped.
        - The first 8K VROM bank is swapped into PPU $0000 when the cart is
           reset.
        - I do not know if all 5 bits of the PRG switcher are used.
           Possibly only three or four are used.
        - Not many games use this mapper, but it's easy to implement, so
           you might as well add it if you're writing a NES emulator.

****************************************************************************

                 Ŀ
   Mapper 9: MMC2  
                   

                                                                     Ŀ
   This mapper is used only on the U.S. versions of Punch-Out (both    
   standard and "Mike Tyson" versions.) Thanks to Paul Robson and      
   Jim Geffre for the mapper information.                              
                                                                       

                Ŀ                                                   Ŀ
   $A000   $AFFF          Ĵ PPPPPPPP                                  
                                                                       
                                                                       
                                                                       
                                         Select 8K ROM bank at $8000   
                                                                       

                Ŀ                                                        Ŀ
   $B000   $CFFF          Ĵ CCCCCCCC                                       
                                                                            
                                                                            
                                                                            
                                         Select 4K VROM bank at PPU $0000   
                                                                            

                Ŀ                                                         Ŀ
   $D000   $DFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $1000    
                                         for use when latch selector is $FD  
                                                                             

                Ŀ                                                         Ŀ
   $E000   $EFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $1000    
                                         for use when latch selector is $FE  
                                                                             

                Ŀ                                                 Ŀ
   $F000   $FFFF    Ĵ xxxxxxxM                                      
                                                                     
                                                                     
                                                                     
                                   Mirroring Select                  
                                    0 - Vertical mirroring           
                                    1 - Horizontal mirroring         
                                                                     

 Notes: - When the cart is first started, the first 8K ROM bank in the cart
           is loaded into $8000, and the LAST 3 8K ROM banks are loaded into
           $A000. These last 8K banks are permanently "hard-wired" to $A000,
           and cannot be swapped.
        - The "latch selector" in question can be swapped by access to PPU
           memory. If PPU $0FD0-$0FDF or $1FD0-$1FDF is accessed, the latch
           selector is $FD. If $0FE0-$0FEF or $1FE0-$1FEF is accessed, the
           latch selector is changed to $FE. These settings take effect
           immediately. The latch contains $FE on reset.

****************************************************************************

                  Ŀ
   Mapper 10: MMC4  
                    

                                                                     Ŀ
   This mapper is used on several Japanese carts such as Fire Emblem   
   and Family War. Thanks to FanWen and Jim Geffre for the mapper      
   information.                                                        
                                                                       

                Ŀ                                                   Ŀ
   $A000   $AFFF          Ĵ PPPPPPPP                                  
                                                                       
                                                                       
                                                                       
                                         Select 16K ROM bank at $8000  
                                                                       

                Ŀ                                                         Ŀ
   $B000   $BFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $0000    
                                         for use when latch #1 is $FD        
                                                                             

                Ŀ                                                         Ŀ
   $C000   $CFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $0000    
                                         for use when latch #1 is $FE        
                                                                             

                Ŀ                                                         Ŀ
   $D000   $DFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $1000    
                                         for use when latch #2 is $FD        
                                                                             

                Ŀ                                                         Ŀ
   $E000   $EFFF          Ĵ CCCCCCCC                                        
                                                                             
                                                                             
                                                                             
                                         Select 4K VROM bank at PPU $1000    
                                         for use when latch #2 is $FE        
                                                                             

                Ŀ                                                 Ŀ
   $F000   $FFFF    Ĵ xxxxxxxM                                      
                                                                     
                                                                     
                                                                     
                                   Mirroring Select                  
                                    0 - Vertical mirroring           
                                    1 - Horizontal mirroring         
                                                                     

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. This last 16K bank is permanently "hard-wired" to $C000,
           and cannot be swapped.
        - The "latches" can be swapped by access to PPU memory. If PPU
           $0FD0-$0FDF is accessed, latch #1 becomes $FD. If $0FE0-$0FEF
           is accessed, it becomes $FE. Latch #2 works in the same manner,
           except the addresses are $1FD0-$1FDF and $1FE0-$1FEF for $FD
           and $FE respectively. These bank switch settings take effect
           immediately. Latches contain $FE on reset.

****************************************************************************

                          Ŀ
   Mapper 11: Color Dreams  
                            

                                                                     Ŀ
   This mapper is used on several unlicensed Color Dreams titles,      
   including Crystal Mines and Pesterminator. I'm not sure if their    
   religious ("Wisdom Tree") games use the same mapper or not.         
                                                                       

                Ŀ                                                   Ŀ
   $8000   $FFFF     Ĵ CCCCPPPP                                       
                                                                       
                          Ĵ         Select 32K ROM bank at $8000      
                                                                       
                                     Select 8K VROM bank at PPU $0000  
                                                                       

 Notes: - When the cart is first started or reset, the first 32K ROM bank
           in the cart is loaded into $8000, and the first 8K VROM bank
           is swapped into PPU $0000.
        - Many games using this mapper are somewhat glitchy.

****************************************************************************

                      Ŀ
   Mapper 15: 100-in-1  
                        

                                                                     Ŀ
   Several hacked Japanese titles use this mapper, such as the         
   100-in-1 pirate cart.                                               
                                                                       

        Ŀ                                                    Ŀ
   $8000     Ĵ SMPPPPPP                                        
                                                                
                             Select 16K ROM bank at $8000       
                             Select next 16K ROM bank at $C000  
                                                                
                             Mirroring Control                  
                              0 - Vertical Mirroring            
                              1 - Horizontal Mirroring          
                                                                
                             Page Swap                          
                              0 - Swap 8K pages at $8000/$A000  
                              1 - Swap 8K pages at $C000/$E000  
                                                                

        Ŀ                                                    Ŀ
   $8001     Ĵ SxPPPPPP                                        
                                                                
                             Select 16K ROM bank at $C000       
                                                                
                             Swap Register                      
                              Swap 8K at $C000 and $E000        
                                                                

        Ŀ                                                    Ŀ
   $8002     Ĵ SxPPPPPP                                        
                                                                
                             Select 8K of a 16K segment at      
                             $8000, $A000, $C000, and $E000.    
                                                                
                             Segment Selector                   
                              0 - Select lower 8K of segment    
                              1 - Select upper 8K of segment    
                                                                

        Ŀ                                                    Ŀ
   $8003     Ĵ SMPPPPPP                                        
                                                                
                             Select 16K ROM bank at $C000       
                                                                
                             Mirroring Control                  
                              0 - Vertical Mirroring            
                              1 - Horizontal Mirroring          
                                                                
                             Swap Register                      
                              Swap 8K at $C000 and $E000        
                                                                

 Notes: - The first 32K of ROM is loaded into $8000 on reset. There is
           8K of VRAM at PPU $0000.

****************************************************************************

                    Ŀ
   Mapper 16: Bandai  
                      

                                                                     Ŀ
   This mapper is used on several Japanese titles by Bandai, such as   
   the DragonBall Z series and the SD Gundam Knight series.            
   As far as I know, it was not used on U.S. games.                    
                                                                       

                      Ŀ                                                 Ŀ
   $6000, $7FF0, $8000    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $0000  
                                                                           

                      Ŀ                                                 Ŀ
   $6001, $7FF1, $8001    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $0400  
                                                                           

                      Ŀ                                                 Ŀ
   $6002, $7FF2, $8002    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $0800  
                                                                           

                      Ŀ                                                 Ŀ
   $6003, $7FF3, $8003    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $0C00  
                                                                           

                      Ŀ                                                 Ŀ
   $6004, $7FF4, $8004    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $1000  
                                                                           

                      Ŀ                                                 Ŀ
   $6005, $7FF5, $8005    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $1400  
                                                                           

                      Ŀ                                                 Ŀ
   $6006, $7FF6, $8006    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $1800  
                                                                           

                      Ŀ                                                 Ŀ
   $6007, $7FF7, $8007    Ĵ CCCCCCCC                                      
                                                                           
                                                                           
                                                                           
                                         Select 1K VROM bank at PPU $1C00  
                                                                           

                      Ŀ                                                 Ŀ
   $6008, $7FF8, $8008    Ĵ PPPPPPPP                                      
                                                                           
                                                                           
                                                                           
                                         Select 16K ROM bank at $8000      
                                                                           

                      Ŀ                                                 Ŀ
   $6009, $7FF9, $8009    Ĵ xxxxxxMM                                      
                                                                           
                                                                           
                                                                           
                                         Mirroring/Page Select             
                                          0 - Horizontal mirroring         
                                          1 - Vertical mirroring           
                                          2 - Mirror pages from $2000      
                                          3 - Mirror pages from $2400      
                                                                           

                      Ŀ                                                 Ŀ
   $600A, $7FFA, $800A    Ĵ xxxxxxxI                                      
                                                                           
                                                                           
                                                                           
                                         IRQ Control Register              
                                          0 - Disable IRQ's                
                                          1 - Enable IRQ's                 
                                                                           

                      Ŀ                                                 Ŀ
   $600B, $7FFB, $800B    Ĵ IIIIIIII                                      
                                                                           
                                                                           
                                                                           
                                         Low byte of IRQ counter           
                                                                           

                      Ŀ                                                 Ŀ
   $600C, $7FFC, $800C    Ĵ IIIIIIII                                      
                                                                           
                                                                           
                                                                           
                                         High byte of IRQ counter          
                                                                           

                      Ŀ                                                 Ŀ
   $600D, $7FFD, $800D    Ĵ EEEEEEEE                                      
                                                                           
                                                                           
                                                                           
                                         EPROM I/O Port                    
                                          I am not sure how this works.    
                                                                           

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. This last 16K bank is permanently "hard-wired" to $C000,
           and it cannot be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - The IRQ counter is decremented at each scanline if active and set
           off when it reaches zero. An IRQ interrupt is executed at that
           point.

****************************************************************************

                       Ŀ
   Mapper 17: FFE F8xxx  
                         

                                                                     Ŀ
   Several hacked Japanese titles use this mapper, such as the hacked  
   versions of Parodius and DragonBall Z 3.                            
                                                                       

        Ŀ                                                     Ŀ
   $42FC    Ĵ xxxPxxxx                                          
                                                                 
                                                                 
                                                                 
                             Unknown                             
                                                                 

        Ŀ                                                     Ŀ
   $42FD    Ĵ xxxMxxxx                                          
                                                                 
                                                                 
                                                                 
                             Unknown                             
                                                                 

        Ŀ                                                     Ŀ
   $42FE    Ĵ xxxPxxxx                                          
                                                                 
                                                                 
                                                                 
                             Page Select                         
                              0 - Mirror pages from PPU $2400    
                              1 - Mirror pages from PPU $2000    
                                                                 

        Ŀ                                                     Ŀ
   $42FF    Ĵ xxxMxxxx                                          
                                                                 
                                                                 
                                                                 
                             Mirroring Select                    
                              0 - Horizontal mirroring           
                              1 - Vertical mirroring             
                                                                 

        Ŀ                                                 Ŀ
   $4501    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            Any value written here will      
                            disable IRQ's.                   
                                                             

        Ŀ                                                 Ŀ
   $4502    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           Low byte of IRQ counter           
                                                             

        Ŀ                                                 Ŀ
   $4503    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           High byte of IRQ counter and      
                           IRQ Control Register 1            
                            Any value written here will      
                            enable IRQ's.                    
                                                             

        Ŀ                                            Ŀ
   $4504    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $8000  
                                                        

        Ŀ                                            Ŀ
   $4505    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $A000  
                                                        

        Ŀ                                            Ŀ
   $4506    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $C000  
                                                        

        Ŀ                                            Ŀ
   $4507    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $E000  
                                                        

        Ŀ                                                 Ŀ
   $4510    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $4511    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                                                             

        Ŀ                                                 Ŀ
   $4512    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $4513    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                                                             

        Ŀ                                                 Ŀ
   $4514    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $4515    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                                                             

        Ŀ                                                 Ŀ
   $4516    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $4517    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - The IRQ counter is incremented at each scanline. When it reaches
           $FFFF, it is reset to zero and an IRQ interrupt is executed.

****************************************************************************

                           Ŀ
   Mapper 18: Jaleco SS8806  
                             

                                                                     Ŀ
   This mapper is used on several Japanese titles by Jaleco, such as   
   Baseball 3. As far as I know, it was not used on U.S. games.                     
                                                                       

        Ŀ                                            Ŀ
   $8000    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $8000  
                            Low 4 bits                  
                                                        

        Ŀ                                            Ŀ
   $8001    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $8000  
                            High 4 bits                 
                                                        

        Ŀ                                            Ŀ
   $8002    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $A000  
                            Low 4 bits                  
                                                        

        Ŀ                                            Ŀ
   $8003    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $A000  
                            High 4 bits                 
                                                        

        Ŀ                                            Ŀ
   $9000    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $C000  
                            Low 4 bits                  
                                                        

        Ŀ                                            Ŀ
   $9001    Ĵ xxxxPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $C000  
                            High 4 bits                 
                                                        

        Ŀ                                                 Ŀ
   $A000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $A001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $A002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $A003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $B002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           Low byte of IRQ counter           
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           Low byte of IRQ counter           
                                                             

        Ŀ                                                 Ŀ
   $E002    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           High byte of IRQ counter          
                                                             

        Ŀ                                                 Ŀ
   $E003    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           High byte of IRQ counter          
                                                             

        Ŀ                                                 Ŀ
   $F000    Ĵ xxxxxxxI                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            1 - Enable IRQ's                 
                                                             

        Ŀ                                                 Ŀ
   $F001    Ĵ xxxxxxxI                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 1            
                            0 - Disable IRQ's                
                            1 - Enable IRQ's                 
                                                             

        Ŀ                                                 Ŀ
   $F002    Ĵ xxxxxxPM                                      
                                                             
                                                             
                                                             
                           Mirroring Control                 
                            0 - Vertical mirroring           
                            1 - Horizontal mirroring         
                                                             
                           One-Screen Mirroring              
                            0 - Regular mirroring            
                            1 - Mirror pages from PPU $2000  
                                                             

        Ŀ                                                 Ŀ
   $F003    Ĵ EEEEEEEE                                      
                                                             
                                                             
                                                             
                           External I/O Port                 
                            I am not sure how this works.    
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000.
        - To use the ROM and VROM switching registers, first write the low
           4 bits of the intended value into the first register, then the
           high 4 bits into the second register. For example, to swap 1K
           VROM bank $B8 to PPU $0400, you'd write $0B into $A003 and $08 to
           $A002. I think that some cartridges do it the other way around,
           writing the low nybble first.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - The IRQ counter is decremented at each scanline. When it reaches
           zero, an IRQ interrupt is executed.
        - This information is untested! I do not have any mapper 18 ROM
           images, unfortunately.

****************************************************************************

                        Ŀ
   Mapper 19: Namcot 106  
                          

                                                                     Ŀ
   This mapper is used on several Japanese titles by Namcot, such as   
   Splatterhouse and Family Stadium '90. As far as I know, it was not  
   used on U.S. games.                                                 
                                                                       

                Ŀ                                                 Ŀ
   $5000   $57FF    Ĵ IIIIIIII                                      
                                                                     
                                                                     
                                                                     
                                   Low byte of IRQ counter           
                                                                     

                Ŀ                                                 Ŀ
   $5800   $5FFF    Ĵ CIIIIIII                                      
                                                                     
                                                                     
                                                                     
                                    High bits of IRQ counter         
                                                                     
                                    IRQ Control Register             
                                     0 - Disable IRQ's               
                                     1 - Enable IRQ's                
                                                                     

                Ŀ                                                 Ŀ
   $8000   $87FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $0000  
                                                                     

                Ŀ                                                 Ŀ
   $8800   $8FFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $0400  
                                                                     

                Ŀ                                                 Ŀ
   $9000   $97FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $0800  
                                                                     

                Ŀ                                                 Ŀ
   $9800   $9FFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $0C00  
                                                                     

                Ŀ                                                 Ŀ
   $A000   $A7FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $1000  
                                                                     

                Ŀ                                                 Ŀ
   $A800   $AFFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $1400  
                                                                     

                Ŀ                                                 Ŀ
   $B000   $B7FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $1800  
                                                                     

                Ŀ                                                 Ŀ
   $B800   $BFFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $1C00  
                                                                     

                Ŀ                                                 Ŀ
   $C000   $C7FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $2000  
                                    A value of $E0 or above will     
                                    use VRAM instead                 
                                                                     

                Ŀ                                                 Ŀ
   $C800   $CFFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $2400  
                                    A value of $E0 or above will     
                                    use VRAM instead                 
                                                                     

                Ŀ                                                 Ŀ
   $D000   $D7FF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $2800  
                                    A value of $E0 or above will     
                                    use VRAM instead                 
                                                                     

                Ŀ                                                 Ŀ
   $D800   $DFFF    Ĵ CCCCCCCC                                      
                                                                     
                                                                     
                                                                     
                                   Select 1K VROM bank at PPU $2C00  
                                    A value of $E0 or above will     
                                    use VRAM instead                 
                                                                     

                Ŀ                                                 Ŀ
   $E000   $E7FF    Ĵ PPPPPPPP                                      
                                                                     
                                                                     
                                                                     
                                   Select 8K ROM bank at $8000       
                                                                     

                Ŀ                                                 Ŀ
   $E800   $EFFF    Ĵ PPPPPPPP                                      
                                                                     
                                                                     
                                                                     
                                   Select 8K ROM bank at $A000       
                                                                     

                Ŀ                                                 Ŀ
   $F000   $F7FF    Ĵ PPPPPPPP                                      
                                                                     
                                                                     
                                                                     
                                   Select 8K ROM bank at $C000       
                                                                     

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - The LAST 8K of VROM is swapped into PPU $0000 on reset, if it
           is present.
        - The IRQ counter is incremented at each scanline. When it reaches
           $7FFF, an IRQ interrupt is executed, but there is no reset.
           This is still preliminary and untested, and I may be wrong on
           this point. Splatterhouse and several other games run fine
           without it.
        - The Namcot 106 mapper supports one or more additional sound
           channels. BioNES supports these. I have no clue how they work.
        - Thanks to Mark Knibbs for correcting several misconceptions about
           this mapper that were included in 0.70.

****************************************************************************

                         Ŀ
   Mapper 21: Konami VRC4  
                           

                                                                     Ŀ
   This mapper is used on several Japanese titles by Konami, such as   
   Wai Wai World 2 and Gradius 2. As far as I know, it was not used    
   on U.S. games.                                                      
                                                                       

        Ŀ                                                 Ŀ
   $8000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                           or $C000 (based on bit 1 of       
                           $9002).                           
                                                             

        Ŀ                                                 Ŀ
   $9000    Ĵ xxxxxxMM                                      
                                                             
                                                             
                                                             
                           Mirroring/Page Select             
                            0 - Vertical mirroring           
                            1 - Horizontal mirroring         
                            2 - Mirror pages from $2400      
                            3 - Mirror pages from $2000      
                                                             

        Ŀ                                                  Ŀ
   $9002    Ĵ xxxxxxPS                                       
                                                              
                                                              
                                                              
                           SaveRAM Toggle                     
                            0 - Disable $6000-$7FFF           
                            1 - Enable $6000-$7FFF            
                                                              
                           $8000 Switching Mode               
                            0 - Switch $8000-$9FFF via $8000  
                            1 - Switch $C000-$DFFF via $8000  
                                                              

        Ŀ                                                 Ŀ
   $9003    Ĵ EEEEEEEE                                      
                                                             
                                                             
                                                             
                           External I/O Port                 
                            I am not sure how this works.    
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $B001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $B004    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B006    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C004    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C006    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D004    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D006    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $E002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $E003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E004    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $E006    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $F000    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           IRQ Counter Register              
                            The IRQ countdown value is       
                            stored here.                     
                                                             

        Ŀ                                                 Ŀ
   $F001    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           IRQ Counter Register              
                            The IRQ countdown value is       
                            stored here. (Apparently is      
                            the same register as $F000.)     
                                                             

        Ŀ                                                 Ŀ
   $F002    Ĵ xxxxxxII                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            0 - Disable IRQ's                
                            2 - Enable IRQ's                 
                            3 - Enable IRQ's                 
                                                             

        Ŀ                                                 Ŀ
   $F003    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 1            
                            Any value written here will      
                            reset the IRQ counter to zero.   
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - To use the VROM switching registers, first write the low
           4 bits of the intended value into the first register, then the
           high 4 bits into the second register. For example, to swap 1K
           VROM bank $B8 to PPU $0800, you'd write $0B into $C002 and $08 to
           $C000. I think that some cartridges do it the other way around,
           writing the low nybble first. Note that this is actually two
           different varieties of mapper combined into one. Gradius 2
           uses the pairs 0-2 and 1-3. Other games (i.e. Wai Wai World 2)
           use the pairs 0-2 and 4-6. In the .NES format these two are
           "shoe-horned" together. fwNES refers to the Gradius 2 style
           as mapper #25 and the Wai Wai World 2 style as mapper #21.
           Marat's standard lists both as #21.
        - The IRQ counter is incremented each 113.75 cycles, which is
           equivalent to one scanline. Unlike a real scanline counter, this
           "scanline-emulated" counter apparently continues to run during
           VBlank. When the IRQ counter value reaches $FF, IRQ's will be
           set off, and the counter is reset.

****************************************************************************

                                Ŀ
   Mapper 22: Konami VRC2 type A  
                                  

                                                                     Ŀ
   This mapper is used on the Japanese title TwinBee 3 by Konami.      
                                                                       

        Ŀ                                                 Ŀ
   $8000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                                                             

        Ŀ                                                 Ŀ
   $9000    Ĵ xxxxxxMM                                      
                                                             
                                                             
                                                             
                           Mirroring/Page Select             
                            0 - Vertical mirroring           
                            1 - Horizontal mirroring         
                            2 - Mirror pages from $2400      
                            3 - Mirror pages from $2000      
               NOTE: I don't have any confidence in the      
                     accuracy of this information.           
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $B001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $D000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $D001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            Shift this value right one bit   
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            Shift this value right one bit   
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 16K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - On reset, the first 8K of VROM is swapped into PPU $0000.

****************************************************************************

                                Ŀ
   Mapper 23: Konami VRC2 type B  
                                  

                                                                     Ŀ
   This mapper is used on several Japanese titles by Konami, such as   
   Contra Japanese and Getsufuu Maden. As far as I know, it was not    
   used on U.S. games.                                                 
                                                                       

        Ŀ                                                 Ŀ
   $8000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                                                             

        Ŀ                                                 Ŀ
   $9000    Ĵ xxxxxxMM                                      
                                                             
                                                             
                                                             
                           Mirroring/Page Select             
                            0 - Vertical mirroring           
                            1 - Horizontal mirroring         
                            2 - Mirror pages from $2400      
                            3 - Mirror pages from $2000      
               NOTE: I don't have any confidence in the      
                     accuracy of this information.           
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $B002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $B003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $C002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $C003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $D002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $D003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                            High 4 bits                      
                                                             

        Ŀ                                                 Ŀ
   $E002    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            Low 4 bits                       
                                                             

        Ŀ                                                 Ŀ
   $E003    Ĵ xxxxCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                            High 4 bits                      
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - To use the VROM switching registers, first write the low
           4 bits of the intended value into the first register, then the
           high 4 bits into the second register. For example, to swap 1K
           VROM bank $B8 to PPU $0800, you'd write $0B into $C001 and $08 to
           $C000. I think that some cartridges do it the other way around,
           writing the low nybble first.

****************************************************************************

                         Ŀ
   Mapper 24: Konami VRC6  
                           

                                                                     Ŀ
   This mapper is used on several Japanese titles by Konami, such as   
   Akumajo Dracula [Castlevania] 3. As far as I know, it was not used  
   on U.S. games.                                                      
                                                                       

        Ŀ                                                 Ŀ
   $8000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 16K ROM bank at $8000      
                                                             

        Ŀ                                               Ŀ
   $B003    Ĵ xxUxMMxx                                    
                                                           
                                                           
                                                           
                         Mirroring/Page Select             
                          0 - Horizontal mirroring         
                          1 - Vertical mirroring           
                          2 - Mirror pages from $2000      
                          3 - Mirror pages from $2400      
                                                           
                         Unknown, but usually set to 1     
                                                           

        Ŀ                                                 Ŀ
   $C000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $C000       
                                                             

        Ŀ                                                 Ŀ
   $D000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $D001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                                                             

        Ŀ                                                 Ŀ
   $D002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $D003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $E001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                                                             

        Ŀ                                                 Ŀ
   $E002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $E003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                                                             

        Ŀ                                                 Ŀ
   $F000    Ĵ IIIIIIII                                      
                                                             
                                                             
                                                             
                           IRQ Counter Register              
                            The IRQ countdown value is       
                            stored here.                     
                                                             

        Ŀ                                                 Ŀ
   $F001    Ĵ xxxxxxxI                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 0            
                            0 - Disable IRQ's                
                            1 - Enable IRQ's                 
                                                             

        Ŀ                                                 Ŀ
   $F002    Ĵ xxxxxxxx                                      
                                                             
                                                             
                                                             
                           IRQ Control Register 1            
                            Any value written here will      
                            reset the IRQ counter to zero.   
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - The IRQ counter is incremented each 113.75 cycles, which is
           equivalent to one scanline. Unlike a real scanline counter, this
           "scanline-emulated" counter apparently continues to run during
           VBlank. When the IRQ counter value reaches $FF, IRQ's will be
           set off, and the counter is reset.
        - There are more registers which I don't understand the usage of
           and which are not detailed here. There's also a custom sound chip,
           the operation of which is unknown to me. As always, any extra
           information is welcome.

****************************************************************************

                        Ŀ
   Mapper 32: Irem G-101  
                          

                                                                     Ŀ
   This mapper is used on several Japanese titles by Irem, such as     
   ImageFight 2. As far as I know, it was not used on U.S. games.                                                  
                                                                       

        Ŀ                                                 Ŀ
   $8FFF    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                           or $C000 (based on bit 1 of       
                           $9FFF).                           
                                                             

        Ŀ                                                  Ŀ
   $9FFF    Ĵ xxxxxxPS                                       
                                                              
                                                              
                                                              
                           Mirroring Switch                   
                            0 - Horizontal mirroring          
                            1 - Vertical mirroring            
                                                              
                           $8FFF Switching Mode               
                            0 - Switch $8000-$9FFF via $8FFF  
                            1 - Switch $C000-$DFFF via $8FFF  
                                                              

        Ŀ                                                 Ŀ
   $AFFF    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $BFF0    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $BFF1    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                                                             

        Ŀ                                                 Ŀ
   $BFF2    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $BFF3    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                                                             

        Ŀ                                                 Ŀ
   $BFF4    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $BFF5    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                                                             

        Ŀ                                                 Ŀ
   $BFF6    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $BFF7    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.

****************************************************************************

                          Ŀ
   Mapper 33: Taito TC0190  
                            

                                                                     Ŀ
   This mapper is used on several Japanese titles by Taito, such as    
   Pon Poko Pon. As far as I know, it was not used on U.S. games.                                                  
                                                                       

        Ŀ                                                 Ŀ
   $8000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $8000       
                                                             

        Ŀ                                                 Ŀ
   $8001    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $8002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $8003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $A001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                                                             

        Ŀ                                                 Ŀ
   $A002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $A003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ UUUUUUUU                                      
                                                             
                                                             
                                                             
                           Unknown                           
                                                             

        Ŀ                                                 Ŀ
   $C001    Ĵ UUUUUUUU                                      
                                                             
                                                             
                                                             
                           Unknown                           
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ RRRRRRRR                                      
                                                             
                                                             
                                                             
                           Reserved                          
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 16K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.

****************************************************************************

                    Ŀ
   Mapper 34: Nina-1  
                      

                                                                     Ŀ
   These two mappers were used on two U.S. games: Deadly Towers and    
   Impossible Mission ][.                                              
                                                                       

        Ŀ                                                 Ŀ
   $7FFD    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 32K ROM bank at $8000      
                                                             

        Ŀ                                                 Ŀ
   $7FFE    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 4K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $7FFF    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 4K VROM bank at PPU $1000  
                                                             

                Ŀ                                                   Ŀ
   $8000   $FFFF          Ĵ PPPPPPPP                                  
                                                                       
                                                                       
                                                                       
                                         Select 32K ROM bank at $8000  
                                                                       

 Notes: - The first 32K ROM bank is swapped into $8000 when the cart is
           started or reset.
        - Carts without VROM (i.e. Deadly Towers) will have 8K of VRAM
           at PPU $0000. Carts with VROM (Impossible Mission 2) have the
           first 8K swapped in at reset. Apparently, this mapper is actually
           a combination of two actual separate mappers. Deadly Towers uses
           only the $8000-$FFFF switching, and Impossible Mission 2 uses
           only the three lower registers.
        - This mapper is fairly easy to implement in a NES emulator.

****************************************************************************

                            Ŀ
   Mapper 64: Tengen RAMBO-1  
                              

                                                                     Ŀ
   This mapper is used on several U.S. unlicensed titles by Tengen.    
   They include Shinobi, Klax, and Skull & Crossbones. Thanks to D     
   for hacking this mapper.                                            
                                                                       

        Ŀ                                                         Ŀ
   $8000    Ĵ CPxxNNNN                                              
                                                                     
                          Command Number                             
                           0 - Select 2 1K VROM pages at PPU $0000   
                           1 - Select 2 1K VROM pages at PPU $0800   
                           2 - Select 1K VROM page at PPU $1000      
                           3 - Select 1K VROM page at PPU $1400      
                           4 - Select 1K VROM page at PPU $1800      
                           5 - Select 1K VROM page at PPU $1C00      
                           6 - Select first switchable ROM page      
                           7 - Select second switchable ROM page     
                           8 - Select 1K VROM page at PPU $0400      
                           9 - Select 1K VROM page at PPU $0C00      
                           15 - Select third switchable ROM page     
                                                                     
                          PRG Address Select        Command Number   
                                                  -#6-  -#7-  -#15-  
                           0 - Enable swapping at $8000/$A000/$C000  
                           1 - Enable swapping at $A000/$C000/$8000  
                                                                     
                          CHR Address Select                         
                           0 - Use normal address for commands 0-5   
                           1 - XOR command 0-5 address with $1000    
                                                                     

        Ŀ                                                 Ŀ
   $8001    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Page Number for Command           
                            Activates the command number     
                            written to bits 0-2 of $8000     
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ xxxxxxxM                                      
                                                             
                                                             
                                                             
                           Mirroring Select                  
                            0 - Horizontal mirroring         
                            1 - Vertical mirroring           
               NOTE: I don't have any confidence in the      
                     accuracy of this information.           
                                                             

 Notes: - Two of the 8K ROM banks in the PRG area are switchable.
           The last page is "hard-wired" to the last 8K bank in
           the cart.
        - At reset, all four 8K banks are set to the last 8K bank
           in the cart.
        - A cart will first write the command and base select number
           to $8000, then the value to be used to $8001.
        - On carts with VROM, the first 8K of VROM is swapped into
           PPU $0000 on reset. On carts without VROM, as always, there
           is 8K of VRAM at PPU $0000.

****************************************************************************

                         Ŀ
   Mapper 65: Irem H-3001  
                           

                                                                     Ŀ
   This mapper is used on several Japanese titles by Irem, such as     
   Daiku no Gensan 2. As far as I know, it was not used on U.S. games.                                             
                                                                       

        Ŀ                                            Ŀ
   $8000    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $8000  
                                                        

        Ŀ                                               Ŀ
   $9003    Ĵ MMMMMMMM                                    
                                                           
                                                           
                                                           
                           Mirroring                       
                            I am not sure how this works.  
                                                           

        Ŀ                                               Ŀ
   $9005    Ĵ IIIIIIII                                    
                                                           
                                                           
                                                           
                           IRQ Control                     
                            I am not sure how this works.  
                                                           

        Ŀ                                               Ŀ
   $9006    Ĵ IIIIIIII                                    
                                                           
                                                           
                                                           
                           IRQ Control                     
                            I am not sure how this works.  
                                                           

        Ŀ                                                 Ŀ
   $A000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $A000       
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $B001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0400  
                                                             

        Ŀ                                                 Ŀ
   $B002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $B003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $0C00  
                                                             

        Ŀ                                                 Ŀ
   $B004    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $B005    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1400  
                                                             

        Ŀ                                                 Ŀ
   $B006    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $B007    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 1K VROM bank at PPU $1C00  
                                                             

        Ŀ                                                 Ŀ
   $C000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 8K ROM bank at $C000       
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 8K of ROM is permanently "hard-wired" and cannot
           be swapped.
        - VROM should NOT be swapped into PPU $0000 when the cartridge is
           started or reset, in order to avoid graphics corruption.
        - Does anyone have info on mirroring or IRQ's for this mapper?

****************************************************************************

                   Ŀ
   Mapper 66: GNROM  
                     

                                                                     Ŀ
   This mapper is used on several Japanese titles, such as             
   DragonBall, and on U.S. titles such as Gumshoe and Dragon Power.                                                
                                                                       

                Ŀ                                                   Ŀ
   $8000   $FFFF     Ĵ xxPPxxCC                                       
                                                                       
                                     Select 8K VROM bank at PPU $0000  
                                                                       
                                     Select 32K ROM bank at $8000      
                                                                       

 Notes: - When the cart is first started or reset, the first 32K ROM bank
           in the cart is loaded into $8000, and the first 8K VROM bank
           is swapped into PPU $0000.
        - This mapper is used on the DragonBall (NOT DragonBallZ) NES
           game. Contrary to popular belief, this mapper is NOT mapper 16!

****************************************************************************

                               Ŀ
   Mapper 68: Sunsoft Mapper #4  
                                 

                                                                      Ŀ
   This mapper is used on the Japanese title AfterBurner ][ by Sunsoft. 
                                                                        

        Ŀ                                                 Ŀ
   $8000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $9000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $A000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $B000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1800  
                                                             

        Ŀ                                                 Ŀ
   $E000    Ĵ xxxxxxMM                                      
                                                             
                                                             
                                                             
                           Mirroring/Page Select             
                            0 - Horizontal mirroring         
                            1 - Vertical mirroring           
                            2 - Mirror pages from $2000      
                            3 - Mirror pages from $2400      
                                                             

        Ŀ                                                 Ŀ
   $F000    Ĵ PPPPPPPP                                      
                                                             
                                                             
                                                             
                           Select 16K ROM bank at $8000      
                                                             

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. The last 16K of ROM is permanently "hard-wired" and cannot
           be swapped.

****************************************************************************

                           Ŀ
   Mapper 69: Sunsoft FME-7  
                             

                                                                     Ŀ
   This mapper is used on several Japanese titles, such as Batman      
   Japanese, and on the U.S. title Batman: Return of the Joker.        
   Thanks to D for hacking this mapper.                                
                                                                       

        Ŀ                                                      Ŀ
   $8000    Ĵ xxxxRRRR                                           
                                                                  
                          Register Number                         
                           0 - Select 1K VROM page at PPU $0000   
                           1 - Select 1K VROM page at PPU $0400   
                           2 - Select 1K VROM page at PPU $0800   
                           3 - Select 1K VROM page at PPU $0C00   
                           4 - Select 1K VROM page at PPU $1000   
                           5 - Select 1K VROM page at PPU $1400   
                           6 - Select 1K VROM page at PPU $1800   
                           7 - Select 1K VROM page at PPU $1C00   
                           8 - Select 8K ROM page at $6000        
                           9 - Select 8K ROM page at $8000        
                          10 - Select 8K ROM page at $A000        
                          11 - Select 8K ROM page at $C000        
                          12 - Select mirroring                   
                          13 - IRQ control                        
                          14 - Low byte of scanline counter       
                          15 - High byte of scanline counter      
                                                                  
               NOTE: I am not sure if the information for         
                      registers 8, 12, 13, 14, and 15 is correct. 
                                                                  
                                                                  

        Ŀ                                                 Ŀ
   $A000    Ĵ VVVVVVVV                                      
                                                             
                                                             
                                                             
                           Register Write                    
                            Activates the command number     
                            written to bits 0-3 of $8000     
                                                             

 Notes: - The last 8K ROM page is permanently "hard-wired" to the last 8K
           ROM page in the cart.
        - This mapper is deployed in a manner similar to that of MMC3. First
           a register number is written to $8000 and then the register
           chosen can be accessed via $A000.
        - Command #8 works in the following manner. The upper 2 bits select
           what is swapped into $6000-$7FFF. If bit 6 is 0, it will be
           ROM, selected from the other bits of the register. If it's 1,
           then the contents depend on bit 7. In this case, if bit 7 is
           1, it will be WRAM. If it's 0, it will be pseudo-random numbers
           (this still hasn't been figured out).

****************************************************************************

                      Ŀ
   Mapper 71: Camerica  
                        

                                                                     Ŀ
   This mapper is used on Camerica's unlicensed NES carts, including   
   Firehawk and Linus Spacehead.                                       
                                                                       

                Ŀ                                                   Ŀ
   $8000   $BFFF          Ĵ UUUUUUUU                                  
                                                                       
                                                                       
                                                                       
                                         Unknown                       
                                                                       

                Ŀ                                                   Ŀ
   $C000   $FFFF          Ĵ PPPPPPPP                                  
                                                                       
                                                                       
                                                                       
                                         Select 16K ROM bank at $8000  
                                                                       

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. This last 16K bank is permanently "hard-wired" to $C000,
           and it cannot be swapped, as far as is known.
        - This mapper has no provisions for VROM; therefore, all carts
           using it have 8K of VRAM at PPU $0000.
        - Many ROMs from these games are incorrectly defined as mapper #2.
           Marat has still not assigned an "official" .NES mapper number
           for this mapper.

****************************************************************************

                             Ŀ
   Mapper 78: Irem 74HC161/32  
                               

                                                Ŀ
   Several Japanese Irem titles use this mapper.  
                                                  

                Ŀ                                                   Ŀ
   $8000   $FFFF     Ĵ CCCCPPPP                                       
                                                                       
                          Ĵ        Select 16K ROM bank at $8000       
                                                                       
                                    Select 8K VROM bank at PPU $0000   
                                                                       

 Notes: - When the cart is first started, the first 16K ROM bank in the cart
           is loaded into $8000, and the LAST 16K ROM bank is loaded into
           $C000. This 16K bank is permanently "hard-wired" to $C000, and it
           cannot be swapped.
        - The first 8K VROM bank may or may not be swapped into $0000 when
           the cart is reset. I have no ROM images to test.

****************************************************************************

                    Ŀ
   Mapper 91: HK-SF3  
                      

                                                                     Ŀ
   This mapper is used on the pirate cart with a title screen reading  
   "Street Fighter 3". It may or may not have been used in other       
   bootleg games. Thanks to Mark Knibbs for information regarding      
   this mapper.                                                        
                                                                       

        Ŀ                                                 Ŀ
   $6000    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0000  
                                                             

        Ŀ                                                 Ŀ
   $6001    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $0800  
                                                             

        Ŀ                                                 Ŀ
   $6002    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1000  
                                                             

        Ŀ                                                 Ŀ
   $6003    Ĵ CCCCCCCC                                      
                                                             
                                                             
                                                             
                           Select 2K VROM bank at PPU $1800  
                                                             

        Ŀ                                            Ŀ
   $7000    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $8000  
                                                        

        Ŀ                                            Ŀ
   $7001    Ĵ PPPPPPPP                                 
                                                        
                                                        
                                                        
                           Select 8K ROM bank at $A000  
                                                        

 Notes: - When the cart is first started, the LAST 16K ROM bank in the cart
           is loaded into both $8000 and $C000. The 16K at $C000 is
           permanently "hard-wired" to $C000 and cannot be swapped.
        - Vertical mirroring is always active.
        - Some of the registers can be accessed from other addresses than
           those listed above. For example, $7000 can also be accessed
           from $7002, $7004, and so on through $7FFA. $7001 can be accessed
           at $7003, $7005, and so on through $7FFB. Similar rules apparently
           are in force for the registers at $6000-$6FFF.
        - This mapper supports IRQ interrupts. I have no clue how.

****************************************************************************

                               Ŀ
   72-in-1 (No Number Assigned)  
                                 

                                              Ŀ
   The 72-in-1 pirate cart uses this mapper.    
                                                

                                                                Ŀ
  A15 A14 A13 A12 A11 A10 A09 A08 A07 A06 A05 A04 A03 A02 A01 A00 
                                                                    
                                                                Ŀ
   1   X   M   S   P   P   P   P   P   H   C   C   C   C   C   C  
                                                                  
                                                                
                                                                
                                            
                                             Select 8K VROM bank at PPU
                                              $0000
                                        
                                             Select upper half (1) or
                                              lower half (0) of 32K PRG
                                              page. This half is mirrored
                                              both to $8000 and $C000.
                                              No effect if page size is set
                                              to 32K.
                    
                                             Select PRG page (size and
                                              location determined by lines
                                              A06 and A12)
                
                                             Select PRG page size. 0 - 32K,
                                              1 - 16K. If the size is 16K,
                                              it's mirrored at both $8000
                                              and $A000. Whether the top
                                              or bottom half of the 32K
                                              chunk is used depends on the
                                              condition of A06.
            
                                             Mirroring select
                                              0 - Vertical mirroring
                                              1 - Horizontal mirroring

 Notes: - This mapper is interesting in that the address written,
           rather than the value, controls the switching. A15-A00 is
           simply the address written in binary notation. For example,
           writing to $A3C7 (1010001111000111 in binary) would select
           horizontal mirroring, 32K PRG bank #7, and 8K CHR bank
           #7. Note that only writes to $8000 and above are valid.
        - Vertical mirroring is probably set at powerup.
        - At reset, the first 32K ROM bank is swapped into $8000,
           and the first 8K VROM bank is swapped into PPU $0000.
        - Thanks to Mark Knibbs for hacking this mapper.

****************************************************************************

                                Ŀ
   110-in-1 (No Number Assigned)  
                                  

                                              Ŀ
   The 110-in-1 pirate cart uses this mapper.   
                                                

                                                                Ŀ
  A15 A14 A13 A12 A11 A10 A09 A08 A07 A06 A05 A04 A03 A02 A01 A00 
                                                                    
                                                                Ŀ
   1   L   M   S   P   P   P   P   P   H   C   C   C   C   C   C  
                                                                  
                                                                
                                                                
                                            
                                             Select 8K VROM bank at PPU
                                              $0000
                                        
                                             Select upper half (1) or
                                              lower half (0) of 32K PRG
                                              page. This half is mirrored
                                              both to $8000 and $C000.
                                              No effect if page size is set
                                              to 32K.
                    
                                             Select PRG page (size and
                                              location determined by lines
                                              A06 and A12)
                
                                             Select PRG page size. 0 - 32K,
                                              1 - 16K. If the size is 16K,
                                              it's mirrored at both $8000
                                              and $A000. Whether the top
                                              or bottom half of the 32K
                                              chunk is used depends on the
                                              condition of A06.
            
                                             Mirroring select
                                              0 - Vertical mirroring
                                              1 - Horizontal mirroring
        
                                             1024K page select
                                              0 - Select pages from first
                                                   megabyte of PRG-ROM
                                              1 - Select pages from second
                                                   megabyte of PRG-ROM

        Ŀ                                            Ŀ
   $5800    Ĵ RRRRRRRR                                 
                                                        
                                                        
                                                        
                           Register #1                  
                            Data storage for menu       
                                                        

        Ŀ                                            Ŀ
   $5801    Ĵ RRRRRRRR                                 
                                                        
                                                        
                                                        
                           Register #1                  
                            Data storage for menu       
                                                        

        Ŀ                                            Ŀ
   $5802    Ĵ RRRRRRRR                                 
                                                        
                                                        
                                                        
                           Register #1                  
                            Data storage for menu       
                                                        

        Ŀ                                            Ŀ
   $5803    Ĵ RRRRRRRR                                 
                                                        
                                                        
                                                        
                           Register #1                  
                            Data storage for menu       
                                                        

 Notes: - This mapper is interesting in that the address written,
           rather than the value, controls the switching. A15-A00 is
           simply the address written in binary notation. For example,
           writing to $A3C7 (1010001111000111 in binary) would select
           horizontal mirroring, 32K PRG bank #7, and 8K CHR bank
           #7. Note that only writes to $8000 and above are valid.
        - Registers can also be accessed at several other values:
           register #0, for example, is also found at $5804, $5808,
           and so on through $5FFC. The same mirroring is true with
           the other registers.
        - Vertical mirroring is probably set at powerup.
        - At reset, the first 32K ROM bank is swapped into $8000,
           and the first 8K VROM bank is swapped into PPU $0000.
           Registers 0 and 2 contain the value 15 ($0F) and registers
           1 and 3 contain a value of zero.
        - Thanks to Mark Knibbs for hacking this mapper.

****************************************************************************

(C) 1997, 1998 Firebug - Cannot be used for commercial gain
